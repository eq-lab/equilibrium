// This file is part of Equilibrium.

// Copyright (C) 2023 EQ Lab.
// SPDX-License-Identifier: GPL-3.0-or-later

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use codec::Codec;
use cumulus_client_cli::CollatorOptions;
use cumulus_client_consensus_aura::{AuraConsensus, BuildAuraConsensusParams, SlotProportion};
use cumulus_client_consensus_common::{
    ParachainBlockImport as TParachainBlockImport, ParachainCandidate, ParachainConsensus,
};
use cumulus_client_service::{
    build_network, prepare_node_config, start_collator, start_full_node, BuildNetworkParams,
    StartCollatorParams, StartFullNodeParams,
};
use cumulus_primitives_core::relay_chain::{Hash as PHash, PersistedValidationData};
use eq_xcm::ParaId;

use cumulus_client_consensus_relay_chain::Verifier as RelayChainVerifier;
use cumulus_client_service::build_relay_chain_interface;
use cumulus_relay_chain_interface::RelayChainInterface;
use futures::lock::Mutex;
use jsonrpsee::RpcModule;
use parachains_common::AuraId;
use sp_core::Pair;

#[cfg(any(feature = "with-eq-runtime", feature = "with-gens-runtime"))]
use crate::chain_spec::IdentifyVariant;
pub use common_runtime::{
    self,
    opaque::{Block, Header},
    AccountId, Balance, BlockNumber, Hash, Index,
};
use sc_client_api::Backend;
use sc_consensus::{
    import_queue::{BasicQueue, Verifier as VerifierT},
    BlockImportParams, ImportQueue,
};
use sc_executor::{HeapAllocStrategy, NativeElseWasmExecutor, DEFAULT_HEAP_ALLOC_STRATEGY};
use sc_network::NetworkBlock;
use sc_network_sync::SyncingService;
use sc_service::{Configuration, PartialComponents, TFullBackend, TFullClient, TaskManager};
use sc_telemetry::{Telemetry, TelemetryHandle, TelemetryWorker, TelemetryWorkerHandle};
use sp_api::{ApiExt, ConstructRuntimeApi};
use sp_consensus_aura::AuraApi;
use sp_core::offchain::OffchainStorage;
use sp_keystore::KeystorePtr;
use sp_runtime::{
    app_crypto::AppCrypto,
    traits::{AccountIdConversion, BlakeTwo256, Header as HeaderT},
};
use std::{marker::PhantomData, sync::Arc, time::Duration};
use substrate_prometheus_endpoint::Registry;

use eq_xcm::relay_interface::storage::known_keys;

type FullClient<RuntimeApi, Executor> =
    TFullClient<Block, RuntimeApi, NativeElseWasmExecutor<Executor>>;
type FullBackend = TFullBackend<Block>;
type ParachainBlockImport<RuntimeApi, Executor> =
    TParachainBlockImport<Block, Arc<FullClient<RuntimeApi, Executor>>, FullBackend>;

pub mod client;
pub use client::Client;

pub trait RuntimeApiCollection:
    sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
    + sp_api::Metadata<Block>
    + sp_api::ApiExt<Block, StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>
    + sp_session::SessionKeys<Block>
    + sp_offchain::OffchainWorkerApi<Block>
    + sp_block_builder::BlockBuilder<Block>
    + cumulus_primitives_core::CollectCollationInfo<Block>
    + sp_consensus_aura::AuraApi<Block, <<AuraId as AppCrypto>::Pair as Pair>::Public>
    + substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Index>
    + pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>
    + equilibrium_curve_amm_rpc::EquilibriumCurveAmmRuntimeApi<Block, Balance>
    + eq_xdot_pool_rpc::EqXdotPoolRuntimeApi<Block, Balance>
    + eq_balances_rpc::EqBalancesRuntimeApi<Block, Balance, AccountId>
{
}

impl<Api> RuntimeApiCollection for Api where
    Api: sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
        + sp_api::Metadata<Block>
        + sp_api::ApiExt<Block, StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>
        + sp_session::SessionKeys<Block>
        + sp_offchain::OffchainWorkerApi<Block>
        + sp_block_builder::BlockBuilder<Block>
        + cumulus_primitives_core::CollectCollationInfo<Block>
        + sp_consensus_aura::AuraApi<Block, <<AuraId as AppCrypto>::Pair as Pair>::Public>
        + substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Index>
        + pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>
        + equilibrium_curve_amm_rpc::EquilibriumCurveAmmRuntimeApi<Block, Balance>
        + eq_xdot_pool_rpc::EqXdotPoolRuntimeApi<Block, Balance>
        + eq_balances_rpc::EqBalancesRuntimeApi<Block, Balance, AccountId>
{
}

const RELAY_CHAIN_SLOT_DURATION_SECS: u64 = 6;

// Native Equilibrium executor instance.
#[cfg(feature = "with-eq-runtime")]
pub struct EquilibriumRuntimeExecutor;
#[cfg(feature = "with-eq-runtime")]
impl sc_executor::NativeExecutionDispatch for EquilibriumRuntimeExecutor {
    type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;

    fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
        eq_node_runtime::api::dispatch(method, data)
    }

    fn native_version() -> sc_executor::NativeVersion {
        eq_node_runtime::native_version()
    }
}

// Native Genshiro executor instance.
#[cfg(feature = "with-gens-runtime")]
pub struct GenshiroRuntimeExecutor;
#[cfg(feature = "with-gens-runtime")]
impl sc_executor::NativeExecutionDispatch for GenshiroRuntimeExecutor {
    type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;

    fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
        gens_node_runtime::api::dispatch(method, data)
    }

    fn native_version() -> sc_executor::NativeVersion {
        gens_node_runtime::native_version()
    }
}

fn init_offchain_execution_id(backend: Arc<FullBackend>) {
    let exec_key = b"execution-id/".to_vec();
    let exec_value: [u8; 32] = rand::random();
    //sp_io::offchain::local_storage_clear(sp_core::offchain::StorageKind::PERSISTENT, &key);
    backend.offchain_storage().unwrap().set(
        sp_runtime::offchain::STORAGE_PREFIX,
        &exec_key,
        &exec_value,
    );
}

/// Builds a new object suitable for chain operations.
pub fn new_chain_ops(
    config: &mut Configuration,
) -> Result<
    (
        Arc<Client>,
        Arc<FullBackend>,
        sc_consensus::BasicQueue<Block, sp_trie::PrefixedMemoryDB<BlakeTwo256>>,
        TaskManager,
    ),
    sc_service::error::Error,
> {
    match &config.chain_spec {
        #[cfg(feature = "with-eq-runtime")]
        spec if spec.is_equilibrium() => {
            new_chain_ops_inner::<eq_node_runtime::RuntimeApi, EquilibriumRuntimeExecutor>(config)
        }
        #[cfg(feature = "with-gens-runtime")]
        spec if spec.is_genshiro() => {
            new_chain_ops_inner::<gens_node_runtime::RuntimeApi, GenshiroRuntimeExecutor>(config)
        }
        _ => panic!("invalid chain spec"),
    }
}

fn new_chain_ops_inner<RuntimeApi, Executor>(
    config: &mut Configuration,
) -> Result<
    (
        Arc<Client>,
        Arc<FullBackend>,
        sc_consensus::BasicQueue<Block, sp_trie::PrefixedMemoryDB<BlakeTwo256>>,
        TaskManager,
    ),
    sc_service::error::Error,
>
where
    Client: From<Arc<FullClient<RuntimeApi, Executor>>>,
    RuntimeApi:
        ConstructRuntimeApi<Block, FullClient<RuntimeApi, Executor>> + Send + Sync + 'static,
    RuntimeApi::RuntimeApi:
        RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>,
    Executor: sc_executor::NativeExecutionDispatch + 'static,
{
    config.keystore = sc_service::config::KeystoreConfig::InMemory;
    let PartialComponents {
        client,
        backend,
        import_queue,
        task_manager,
        ..
    } = new_partial::<RuntimeApi, Executor, _>(config, build_import_queue::<_, _>)?;
    Ok((
        Arc::new(Client::from(client)),
        backend,
        import_queue,
        task_manager,
    ))
}

pub fn new_partial<RuntimeApi, Executor, BIQ>(
    config: &Configuration,
    build_import_queue: BIQ,
) -> Result<
    PartialComponents<
        FullClient<RuntimeApi, Executor>,
        FullBackend,
        (),
        sc_consensus::DefaultImportQueue<Block, FullClient<RuntimeApi, Executor>>,
        sc_transaction_pool::FullPool<Block, FullClient<RuntimeApi, Executor>>,
        (
            ParachainBlockImport<RuntimeApi, Executor>,
            Option<Telemetry>,
            Option<TelemetryWorkerHandle>,
        ),
    >,
    sc_service::Error,
>
where
    RuntimeApi:
        ConstructRuntimeApi<Block, FullClient<RuntimeApi, Executor>> + Send + Sync + 'static,
    RuntimeApi::RuntimeApi: RuntimeApiCollection,
    Executor: sc_executor::NativeExecutionDispatch + 'static,
    BIQ: FnOnce(
        Arc<FullClient<RuntimeApi, Executor>>,
        ParachainBlockImport<RuntimeApi, Executor>,
        &Configuration,
        Option<TelemetryHandle>,
        &TaskManager,
    ) -> Result<
        sc_consensus::DefaultImportQueue<Block, FullClient<RuntimeApi, Executor>>,
        sc_service::Error,
    >,
{
    let telemetry = config
        .telemetry_endpoints
        .clone()
        .filter(|x| !x.is_empty())
        .map(|endpoints| -> Result<_, sc_telemetry::Error> {
            let worker = TelemetryWorker::new(16)?;
            let telemetry = worker.handle().new_telemetry(endpoints);
            Ok((worker, telemetry))
        })
        .transpose()?;

    let executor = {
        let heap_pages = config
            .default_heap_pages
            .map_or(DEFAULT_HEAP_ALLOC_STRATEGY, |h| HeapAllocStrategy::Static {
                extra_pages: h as _,
            });

        let wasm_executor = sc_executor::WasmExecutor::builder()
            .with_execution_method(config.wasm_method)
            .with_max_runtime_instances(config.max_runtime_instances)
            .with_runtime_cache_size(config.runtime_cache_size)
            .with_onchain_heap_alloc_strategy(heap_pages)
            .with_offchain_heap_alloc_strategy(heap_pages)
            .build();
        sc_executor::NativeElseWasmExecutor::<Executor>::new_with_wasm_executor(wasm_executor)
    };

    let (client, backend, keystore_container, task_manager) =
        sc_service::new_full_parts::<Block, RuntimeApi, _>(
            &config,
            telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
            executor,
        )?;
    let client = Arc::new(client);

    let telemetry_worker_handle = telemetry.as_ref().map(|(worker, _)| worker.handle());

    let telemetry = telemetry.map(|(worker, telemetry)| {
        task_manager
            .spawn_handle()
            .spawn("telemetry", None, worker.run());
        telemetry
    });

    let transaction_pool = sc_transaction_pool::BasicPool::new_full(
        config.transaction_pool.clone(),
        config.role.is_authority().into(),
        config.prometheus_registry(),
        task_manager.spawn_essential_handle(),
        client.clone(),
    );

    let block_import = ParachainBlockImport::new(client.clone(), backend.clone());

    let import_queue = build_import_queue(
        client.clone(),
        block_import.clone(),
        config,
        telemetry.as_ref().map(|telemetry| telemetry.handle()),
        &task_manager,
    )?;

    let params = PartialComponents {
        backend,
        client,
        import_queue,
        keystore_container,
        task_manager,
        transaction_pool,
        select_chain: (),
        other: (block_import, telemetry, telemetry_worker_handle),
    };

    Ok(params)
}

/// Start a node with the given parachain `Configuration` and relay chain `Configuration`.
///
/// This is the actual implementation that is abstract over the executor and the runtime api.
#[sc_tracing::logging::prefix_logs_with("Equilibrium")]
async fn start_node_impl<RuntimeApi, Executor, RB, BIQ, BIC>(
    parachain_config: Configuration,
    polkadot_config: Configuration,
    collator_options: CollatorOptions,
    para_id: ParaId,
    _rpc_ext_builder: RB,
    build_import_queue: BIQ,
    build_consensus: BIC,
    hwbench: Option<sc_sysinfo::HwBench>,
) -> sc_service::error::Result<(TaskManager, Arc<FullClient<RuntimeApi, Executor>>)>
where
    RuntimeApi:
        ConstructRuntimeApi<Block, FullClient<RuntimeApi, Executor>> + Send + Sync + 'static,
    RuntimeApi::RuntimeApi: RuntimeApiCollection,
    Executor: sc_executor::NativeExecutionDispatch + 'static,
    RB: Fn(
            Arc<FullClient<RuntimeApi, Executor>>,
        ) -> Result<jsonrpsee::RpcModule<()>, sc_service::Error>
        + 'static,
    BIQ: FnOnce(
        Arc<FullClient<RuntimeApi, Executor>>,
        ParachainBlockImport<RuntimeApi, Executor>,
        &Configuration,
        Option<TelemetryHandle>,
        &TaskManager,
    ) -> Result<
        sc_consensus::DefaultImportQueue<Block, FullClient<RuntimeApi, Executor>>,
        sc_service::Error,
    >,
    BIC: FnOnce(
        Arc<FullClient<RuntimeApi, Executor>>,
        ParachainBlockImport<RuntimeApi, Executor>,
        Option<&Registry>,
        Option<TelemetryHandle>,
        &TaskManager,
        Arc<dyn RelayChainInterface>,
        Arc<sc_transaction_pool::FullPool<Block, FullClient<RuntimeApi, Executor>>>,
        Arc<SyncingService<Block>>,
        KeystorePtr,
        bool,
    ) -> Result<Box<dyn ParachainConsensus<Block>>, sc_service::Error>,
{
    let parachain_config = prepare_node_config(parachain_config);

    let params = new_partial::<RuntimeApi, Executor, BIQ>(&parachain_config, build_import_queue)?;
    let (block_import, mut telemetry, telemetry_worker_handle) = params.other;

    let client = params.client.clone();
    let backend = params.backend.clone();

    let mut task_manager = params.task_manager;

    let (relay_chain_interface, collator_key) = build_relay_chain_interface(
        polkadot_config,
        &parachain_config,
        telemetry_worker_handle,
        &mut task_manager,
        collator_options.clone(),
        hwbench.clone(),
    )
    .await
    .map_err(|e| sc_service::Error::Application(Box::new(e) as Box<_>))?;

    let force_authoring = parachain_config.force_authoring;
    let validator = parachain_config.role.is_authority();
    let prometheus_registry = parachain_config.prometheus_registry().cloned();
    let transaction_pool = params.transaction_pool.clone();
    let import_queue_service = params.import_queue.service();
    let (network, system_rpc_tx, tx_handler_controller, start_network, sync_service) =
        build_network(BuildNetworkParams {
            parachain_config: &parachain_config,
            client: client.clone(),
            transaction_pool: transaction_pool.clone(),
            para_id,
            spawn_handle: task_manager.spawn_handle(),
            relay_chain_interface: relay_chain_interface.clone(),
            import_queue: params.import_queue,
        })
        .await?;

    if parachain_config.offchain_worker.enabled || validator {
        sc_service::build_offchain_workers(
            &parachain_config,
            task_manager.spawn_handle(),
            client.clone(),
            network.clone(),
        );
        init_offchain_execution_id(backend.clone());
    }

    let rpc_builder = {
        let client = client.clone();
        let transaction_pool = transaction_pool.clone();

        Box::new(move |deny_unsafe, _| {
            let deps = crate::rpc::FullDeps {
                client: client.clone(),
                pool: transaction_pool.clone(),
                deny_unsafe,
            };

            crate::rpc::create_full(deps).map_err(Into::into)
        })
    };

    sc_service::spawn_tasks(sc_service::SpawnTasksParams {
        rpc_builder,
        client: client.clone(),
        transaction_pool: transaction_pool.clone(),
        task_manager: &mut task_manager,
        config: parachain_config,
        keystore: params.keystore_container.keystore(),
        backend: backend.clone(),
        network: network.clone(),
        sync_service: sync_service.clone(),
        system_rpc_tx,
        tx_handler_controller,
        telemetry: telemetry.as_mut(),
    })?;

    let announce_block = {
        let sync_service = sync_service.clone();
        Arc::new(move |hash, data| sync_service.announce_block(hash, data))
    };

    let relay_chain_slot_duration = Duration::from_secs(RELAY_CHAIN_SLOT_DURATION_SECS);
    let overseer_handle = relay_chain_interface
        .overseer_handle()
        .map_err(|e| sc_service::Error::Application(Box::new(e)))?;

    if validator {
        let parachain_consensus = build_consensus(
            client.clone(),
            block_import,
            prometheus_registry.as_ref(),
            telemetry.as_ref().map(|t| t.handle()),
            &task_manager,
            relay_chain_interface.clone(),
            transaction_pool,
            sync_service.clone(),
            params.keystore_container.keystore(),
            force_authoring,
        )?;

        let spawner = task_manager.spawn_handle();

        let params = StartCollatorParams {
            para_id,
            block_status: client.clone(),
            announce_block,
            client: client.clone(),
            task_manager: &mut task_manager,
            relay_chain_interface,
            spawner,
            parachain_consensus,
            import_queue: import_queue_service,
            collator_key: collator_key.expect("Command line arguments do not allow this. qed"),
            relay_chain_slot_duration,
            recovery_handle: Box::new(overseer_handle),
            sync_service,
        };

        start_collator(params).await?;
    } else {
        let params = StartFullNodeParams {
            client: client.clone(),
            announce_block,
            task_manager: &mut task_manager,
            para_id,
            relay_chain_interface,
            relay_chain_slot_duration,
            import_queue: import_queue_service,
            recovery_handle: Box::new(overseer_handle),
            sync_service,
        };

        start_full_node(params)?;
    }

    start_network.start_network();

    Ok((task_manager, client))
}

/// Start an Equilibrium parachain node.
pub async fn start_node<RuntimeApi, Executor>(
    parachain_config: Configuration,
    polkadot_config: Configuration,
    collator_options: CollatorOptions,
    para_id: ParaId,
    hwbench: Option<sc_sysinfo::HwBench>,
) -> sc_service::error::Result<(TaskManager, Arc<FullClient<RuntimeApi, Executor>>)>
where
    RuntimeApi:
        ConstructRuntimeApi<Block, FullClient<RuntimeApi, Executor>> + Send + Sync + 'static,
    RuntimeApi::RuntimeApi: RuntimeApiCollection,
    Executor: sc_executor::NativeExecutionDispatch + 'static,
{
    start_node_impl::<RuntimeApi, Executor, _, _, _>(
        parachain_config,
        polkadot_config,
        collator_options,
        para_id,
        |_| Ok(RpcModule::new(())),
        build_import_queue::<_, _>,
        |client,
         block_import,
         prometheus_registry,
         telemetry,
         task_manager,
         relay_chain_interface,
         transaction_pool,
         sync_oracle,
         keystore,
         force_authoring| {
			let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;

			let proposer_factory = sc_basic_authorship::ProposerFactory::with_proof_recording(
				task_manager.spawn_handle(),
				client.clone(),
				transaction_pool,
				prometheus_registry,
				telemetry.clone(),
			);

			Ok(AuraConsensus::build::<<AuraId as AppCrypto>::Pair, _, _, _, _, _, _>(
                BuildAuraConsensusParams {
                    proposer_factory,
                    create_inherent_data_providers:
                        move |_, (relay_parent, validation_data)| {
                            let relay_chain_interface = relay_chain_interface.clone();
                            async move {
                                let parachain_inherent = crate::custom_client_side::create_at(
                                    relay_parent,
                                    &relay_chain_interface,
                                    &validation_data,
                                    para_id,
                                    [
                                        // Active staking era
                                        known_keys::STAKING_CURRENT_ERA.to_vec(),
                                        // Sovereign account ledger storage
                                        known_keys::staking_ledger_maybe_derivative(
                                            para_id.into_account_truncating(),
                                            None,
                                        ),
                                    ],
                                )
                                .await;
                                let timestamp =
                                    sp_timestamp::InherentDataProvider::from_system_time();

                                let slot =
                                sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
                                    *timestamp,
                                    slot_duration,
                                );

                                let parachain_inherent =
                                    parachain_inherent.ok_or_else(|| {
                                        Box::<dyn std::error::Error + Send + Sync>::from(
                                            "Failed to create parachain inherent",
                                        )
                                    })?;
                                Ok((slot, timestamp, parachain_inherent))
                            }
                        },
                    block_import,
                    para_client: client,
                    backoff_authoring_blocks: Option::<()>::None,
                    sync_oracle,
                    keystore,
                    force_authoring,
                    slot_duration,
                    // We got around 500ms for proposing
                    block_proposal_slot_portion: SlotProportion::new(1f32 / 24f32),
                    // And a maximum of 750ms if slots are skipped
                    max_block_proposal_slot_portion: Some(SlotProportion::new(1f32 / 16f32)),
                    telemetry,
                },
            ))
        },
        hwbench,
    )
    .await
}

enum BuildOnAccess<R> {
    Uninitialized(Option<Box<dyn FnOnce() -> R + Send + Sync>>),
    Initialized(R),
}

impl<R> BuildOnAccess<R> {
    fn get_mut(&mut self) -> &mut R {
        loop {
            match self {
                Self::Uninitialized(f) => {
                    *self = Self::Initialized((f.take().unwrap())());
                }
                Self::Initialized(ref mut r) => return r,
            }
        }
    }
}

/// Special [`ParachainConsensus`] implementation that waits for the upgrade from
/// shell to a parachain runtime that implements Aura.
struct WaitForAuraConsensus<Client, AuraId> {
    client: Arc<Client>,
    aura_consensus: Arc<Mutex<BuildOnAccess<Box<dyn ParachainConsensus<Block>>>>>,
    relay_chain_consensus: Arc<Mutex<Box<dyn ParachainConsensus<Block>>>>,
    _phantom: PhantomData<AuraId>,
}

impl<Client, AuraId> Clone for WaitForAuraConsensus<Client, AuraId> {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            aura_consensus: self.aura_consensus.clone(),
            relay_chain_consensus: self.relay_chain_consensus.clone(),
            _phantom: PhantomData,
        }
    }
}

#[async_trait::async_trait]
impl<Client, AuraId> ParachainConsensus<Block> for WaitForAuraConsensus<Client, AuraId>
where
    Client: sp_api::ProvideRuntimeApi<Block> + Send + Sync,
    Client::Api: AuraApi<Block, AuraId>,
    AuraId: Send + Codec + Sync,
{
    async fn produce_candidate(
        &mut self,
        parent: &Header,
        relay_parent: PHash,
        validation_data: &PersistedValidationData,
    ) -> Option<ParachainCandidate<Block>> {
        if self
            .client
            .runtime_api()
            .has_api::<dyn AuraApi<Block, AuraId>>(parent.hash())
            .unwrap_or(false)
        {
            self.aura_consensus
                .lock()
                .await
                .get_mut()
                .produce_candidate(parent, relay_parent, validation_data)
                .await
        } else {
            self.relay_chain_consensus
                .lock()
                .await
                .produce_candidate(parent, relay_parent, validation_data)
                .await
        }
    }
}

struct Verifier<Client, AuraId> {
    client: Arc<Client>,
    aura_verifier: BuildOnAccess<Box<dyn VerifierT<Block>>>,
    relay_chain_verifier: Box<dyn VerifierT<Block>>,
    _phantom: PhantomData<AuraId>,
}

#[async_trait::async_trait]
impl<Client, AuraId> VerifierT<Block> for Verifier<Client, AuraId>
where
    Client: sp_api::ProvideRuntimeApi<Block> + Send + Sync,
    Client::Api: AuraApi<Block, AuraId>,
    AuraId: Send + Sync + Codec,
{
    async fn verify(
        &mut self,
        block_import: BlockImportParams<Block, ()>,
    ) -> Result<BlockImportParams<Block, ()>, String> {
        if self
            .client
            .runtime_api()
            .has_api::<dyn AuraApi<Block, AuraId>>(*block_import.header.parent_hash())
            .unwrap_or(false)
        {
            self.aura_verifier.get_mut().verify(block_import).await
        } else {
            self.relay_chain_verifier.verify(block_import).await
        }
    }
}

/// Build the import queue for the runtime.
pub fn build_import_queue<RuntimeApi, Executor>(
    client: Arc<FullClient<RuntimeApi, Executor>>,
    block_import: ParachainBlockImport<RuntimeApi, Executor>,
    config: &Configuration,
    telemetry_handle: Option<TelemetryHandle>,
    task_manager: &TaskManager,
) -> Result<
    sc_consensus::DefaultImportQueue<Block, FullClient<RuntimeApi, Executor>>,
    sc_service::Error,
>
where
    RuntimeApi:
        ConstructRuntimeApi<Block, FullClient<RuntimeApi, Executor>> + Send + Sync + 'static,
    RuntimeApi::RuntimeApi: RuntimeApiCollection,
    Executor: sc_executor::NativeExecutionDispatch + 'static,
{
    let client2 = client.clone();

    let aura_verifier = move || {
        let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client2).unwrap();

        Box::new(cumulus_client_consensus_aura::build_verifier::<
            <AuraId as AppCrypto>::Pair,
            _,
            _,
            _,
        >(
            cumulus_client_consensus_aura::BuildVerifierParams {
                client: client2.clone(),
                create_inherent_data_providers: move |_, _| async move {
                    let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

                    let slot =
							sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
								*timestamp,
								slot_duration,
							);

                    Ok((slot, timestamp))
                },
                telemetry: telemetry_handle,
            },
        )) as Box<_>
    };

    let relay_chain_verifier = Box::new(RelayChainVerifier::new(client.clone(), |_, _| async {
        Ok(())
    })) as Box<_>;

    let verifier = Verifier {
        client: client.clone(),
        relay_chain_verifier,
        aura_verifier: BuildOnAccess::Uninitialized(Some(Box::new(aura_verifier))),
        _phantom: PhantomData,
    };

    let registry = config.prometheus_registry().clone();
    let spawner = task_manager.spawn_essential_handle();

    Ok(BasicQueue::new(
        verifier,
        Box::new(block_import),
        None,
        &spawner,
        registry,
    ))
}
