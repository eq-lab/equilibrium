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

use crate::{
    chain_spec::*,
    cli::{Cli, RelayChainCli, Subcommand},
    service,
};
use codec::Encode;
use cumulus_client_cli::generate_genesis_block;
use eq_xcm::ParaId;
#[cfg(feature = "runtime-benchmarks")]
use frame_benchmarking_cli::{BenchmarkCmd, SUBSTRATE_REFERENCE_HARDWARE};
use log::info;
use sc_cli::{
    ChainSpec, CliConfiguration, DefaultConfigurationValues, ImportParams, KeystoreParams,
    NetworkParams, Result, RuntimeVersion, SharedParams, SubstrateCli,
};
use sc_service::config::{BasePath, PrometheusConfig};
use sp_core::hexdisplay::HexDisplay;
use sp_runtime::traits::{AccountIdConversion, Block as BlockT};
use std::{io::Write, net::SocketAddr, path::PathBuf};

fn load_spec(id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
    Ok(match id {
        #[cfg(feature = "with-eq-runtime")]
        "dev" | "equilibrium-dev" | "eq-dev" => Box::new(equilibrium::development_config()),
        #[cfg(feature = "with-eq-runtime")]
        "equilibrium-local" => Box::new(equilibrium::local_testnet_config()),
        #[cfg(feature = "with-eq-runtime")]
        "equilibrium" => Box::new(equilibrium::mainnet_config()?),
        #[cfg(all(feature = "with-gens-runtime", not(feature = "with-eq-runtime")))]
        "dev" => Box::new(genshiro::development_config()),
        #[cfg(feature = "with-gens-runtime")]
        "genshiro-dev" | "gens-dev" => Box::new(genshiro::development_config()),
        #[cfg(feature = "with-gens-runtime")]
        "genshiro-local" => Box::new(genshiro::local_testnet_config()),
        #[cfg(feature = "with-gens-runtime")]
        "genshiro" => Box::new(genshiro::mainnet_config()?),
        path => {
            let path = PathBuf::from(path);

            let starts_with = |prefix: &str| {
                path.file_name()
                    .and_then(|f| f.to_str().map(|s| s.starts_with(&prefix)))
                    .unwrap_or(false)
            };

            #[cfg(feature = "with-eq-runtime")]
            if cfg!(not(feature = "with-gens-runtime")) || starts_with("eq") {
                return Ok(Box::new(equilibrium::ChainSpec::from_json_file(path)?));
            }
            #[cfg(feature = "with-gens-runtime")]
            if cfg!(not(feature = "with-eq-runtime")) || starts_with("gens") {
                return Ok(Box::new(genshiro::ChainSpec::from_json_file(path)?));
            }
            Box::new(RawChainSpec::from_json_file(path)?)
        }
    })
}

impl SubstrateCli for Cli {
    fn impl_name() -> String {
        "Substrate Node".into()
    }

    fn impl_version() -> String {
        env!("SUBSTRATE_CLI_IMPL_VERSION").into()
    }

    fn description() -> String {
        env!("CARGO_PKG_DESCRIPTION").into()
    }

    fn author() -> String {
        env!("CARGO_PKG_AUTHORS").into()
    }

    fn support_url() -> String {
        "support.anonymous.an".into()
    }

    fn copyright_start_year() -> i32 {
        2017
    }

    fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
        load_spec(id)
    }

    fn native_runtime_version(spec: &Box<dyn ChainSpec>) -> &'static RuntimeVersion {
        #[cfg(feature = "with-eq-runtime")]
        if spec.is_equilibrium() {
            return &eq_node_runtime::VERSION;
        }

        #[cfg(feature = "with-gens-runtime")]
        if spec.is_genshiro() {
            return &gens_node_runtime::VERSION;
        }

        panic!("invalid chain spec");
    }
}

impl SubstrateCli for RelayChainCli {
    fn impl_name() -> String {
        "Polkadot collator".into()
    }

    fn impl_version() -> String {
        env!("SUBSTRATE_CLI_IMPL_VERSION").into()
    }

    fn description() -> String {
        format!(
            "Polkadot collator\n\nThe command-line arguments provided first will be \
		passed to the parachain node, while the arguments provided after -- will be passed \
		to the relay chain node.\n\n\
		{} [parachain-args] -- [relay_chain-args]",
            Self::executable_name()
        )
    }

    fn author() -> String {
        env!("CARGO_PKG_AUTHORS").into()
    }

    fn support_url() -> String {
        "https://github.com/paritytech/cumulus/issues/new".into()
    }

    fn copyright_start_year() -> i32 {
        2017
    }

    fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
        polkadot_cli::Cli::from_iter([RelayChainCli::executable_name().to_string()].iter())
            .load_spec(id)
    }

    fn native_runtime_version(chain_spec: &Box<dyn ChainSpec>) -> &'static RuntimeVersion {
        polkadot_cli::Cli::native_runtime_version(chain_spec)
    }
}

fn extract_genesis_wasm(chain_spec: &Box<dyn sc_service::ChainSpec>) -> Result<Vec<u8>> {
    let mut storage = chain_spec.build_storage()?;

    storage
        .top
        .remove(sp_core::storage::well_known_keys::CODE)
        .ok_or_else(|| "Could not find wasm file in genesis state!".into())
}

/// Parse and run command line arguments
pub fn run() -> sc_cli::Result<()> {
    let cli = Cli::from_args();

    match &cli.subcommand {
        Some(Subcommand::Key(cmd)) => cmd.run(&cli),
        Some(Subcommand::BuildSpec(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
        }
        Some(Subcommand::CheckBlock(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|mut config| {
                let (client, _, import_queue, task_manager) = service::new_chain_ops(&mut config)?;
                Ok((cmd.run(client, import_queue), task_manager))
            })
        }
        Some(Subcommand::ExportBlocks(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|mut config| {
                let (client, _, _, task_manager) = service::new_chain_ops(&mut config)?;
                Ok((cmd.run(client, config.database), task_manager))
            })
        }
        Some(Subcommand::ExportState(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|mut config| {
                let (client, _, _, task_manager) = service::new_chain_ops(&mut config)?;
                Ok((cmd.run(client, config.chain_spec), task_manager))
            })
        }
        Some(Subcommand::ImportBlocks(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|mut config| {
                let (client, _, import_queue, task_manager) = service::new_chain_ops(&mut config)?;
                Ok((cmd.run(client, import_queue), task_manager))
            })
        }
        Some(Subcommand::PurgeChain(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.sync_run(|config| {
                let polkadot_cli = RelayChainCli::new(
                    &config,
                    [RelayChainCli::executable_name().to_string()]
                        .iter()
                        .chain(cli.relaychain_args.iter()),
                );

                let polkadot_config = SubstrateCli::create_configuration(
                    &polkadot_cli,
                    &polkadot_cli,
                    config.tokio_handle.clone(),
                )
                .map_err(|err| format!("Relay chain argument error: {}", err))?;

                cmd.run(config, polkadot_config)
            })
        }
        Some(Subcommand::Revert(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|mut config| {
                let (client, backend, _, task_manager) = service::new_chain_ops(&mut config)?;
                Ok((cmd.run(client, backend, None), task_manager))
            })
        }
        Some(Subcommand::ExportGenesisState(params)) => {
            let mut builder = sc_cli::LoggerBuilder::new("");
            builder.with_profiling(sc_tracing::TracingReceiver::Log, "");
            let _ = builder.init();

            let spec = load_spec(&params.chain.clone().unwrap_or_default())?;
            let state_version = Cli::native_runtime_version(&spec).state_version();

            let block: crate::service::Block = generate_genesis_block(&*spec, state_version)?;
            let raw_header = block.header().encode();
            let output_buf = if params.raw {
                raw_header
            } else {
                format!("0x{:?}", HexDisplay::from(&block.header().encode())).into_bytes()
            };

            if let Some(output) = &params.output {
                std::fs::write(output, output_buf)?;
            } else {
                std::io::stdout().write_all(&output_buf)?;
            }

            Ok(())
        }
        Some(Subcommand::ExportGenesisWasm(params)) => {
            let mut builder = sc_cli::LoggerBuilder::new("");
            builder.with_profiling(sc_tracing::TracingReceiver::Log, "");
            let _ = builder.init();

            let raw_wasm_blob =
                extract_genesis_wasm(&cli.load_spec(&params.chain.clone().unwrap_or_default())?)?;
            let output_buf = if params.raw {
                raw_wasm_blob
            } else {
                format!("0x{:?}", HexDisplay::from(&raw_wasm_blob)).into_bytes()
            };

            if let Some(output) = &params.output {
                std::fs::write(output, output_buf)?;
            } else {
                std::io::stdout().write_all(&output_buf)?;
            }

            Ok(())
        }
        #[cfg(feature = "runtime-benchmarks")]
        Some(Subcommand::Benchmark(cmd)) => {
            let runner = cli.create_runner(cmd)?;

            match cmd {
                BenchmarkCmd::Pallet(cmd) => match &runner.config().chain_spec {
                    #[cfg(feature = "with-eq-runtime")]
                    spec if spec.is_equilibrium() => runner.sync_run(|config| {
                        cmd.run::<service::Block, service::EquilibriumRuntimeExecutor>(config)
                    }),
                    #[cfg(feature = "with-gens-runtime")]
                    spec if spec.is_genshiro() => runner.sync_run(|config| {
                        cmd.run::<service::Block, service::GenshiroRuntimeExecutor>(config)
                    }),
                    _ => panic!("invalid chain spec"),
                },
                BenchmarkCmd::Storage(cmd) => runner.sync_run(|mut config| {
                    let (client, backend, _, _) = service::new_chain_ops(&mut config)?;

                    let db = backend.expose_db();
                    let storage = backend.expose_storage();
                    cmd.run(config, client, db, storage)
                }),
                BenchmarkCmd::Block(cmd) => match &runner.config().chain_spec {
                    #[cfg(feature = "with-eq-runtime")]
                    spec if spec.is_equilibrium() => runner.sync_run(|mut config| {
                        let params =
                            service::new_partial::<
                                eq_node_runtime::RuntimeApi,
                                service::EquilibriumRuntimeExecutor,
                                _,
                            >(&mut config, service::build_import_queue)?;

                        cmd.run(params.client)
                    }),
                    #[cfg(feature = "with-gens-runtime")]
                    spec if spec.is_genshiro() => runner.sync_run(|mut config| {
                        let params =
                            service::new_partial::<
                                gens_node_runtime::RuntimeApi,
                                service::GenshiroRuntimeExecutor,
                                _,
                            >(&mut config, service::build_import_queue)?;

                        cmd.run(params.client)
                    }),
                    _ => panic!("invalid chain spec"),
                },
                BenchmarkCmd::Machine(cmd) => {
                    runner.sync_run(|config| cmd.run(&config, SUBSTRATE_REFERENCE_HARDWARE.clone()))
                }
                #[allow(unreachable_patterns)]
                _ => Err("Unsupported benchmarking command".into()),
            }
        }
        #[cfg(not(feature = "runtime-benchmarks"))]
        Some(Subcommand::Benchmark(_)) => {
            Err("Benchmarking wasn't enabled when building the node. \
        You can enable it with `--features runtime-benchmarks`."
                .into())
        }
        #[cfg(feature = "try-runtime")]
        Some(Subcommand::TryRuntime(_cmd)) => {
            // let runner = cli.create_runner(cmd)?;
            // let registry = &runner
            //     .config()
            //     .prometheus_config
            //     .as_ref()
            //     .map(|cfg| &cfg.registry);
            // let task_manager =
            //     TaskManager::new(runner.config().tokio_handle.clone(), *registry)
            //         .map_err(|e| format!("Error: {:?}", e))?;
            // match runner.config().chain_spec {
            //     #[cfg(feature = "with-er-runtime")]
            //     spec if spec.is_equilibrium() => {
            //         runner.async_run(|config| {
            //             Ok((
            //                 cmd.run::<Block, service::EquilibriumRuntimeExecutor>(config),
            //                 task_manager,
            //             ))
            //         })
            //     },
            //     #[cfg(feature = "with-gens-runtime")]
            //     spec if spec.is_genshiro() => {
            //         runner.async_run(|config| {
            //             Ok((
            //                 cmd.run::<Block, service::GenchiroRuntimeExecutor>(config),
            //                 task_manager,
            //             ))
            //         })
            //     }
            // 	_ => panic!("invalid chain spec"),
            // }
            Err("TryRuntime not supported byt equilibrium runtime".into())
        }
        #[cfg(not(feature = "try-runtime"))]
        Some(Subcommand::TryRuntime) => Err("TryRuntime wasn't enabled when building the node. \
				You can enable it at build time with `--features try-runtime`."
            .into()),
        None => {
            let runner = cli.create_runner(&cli.run.normalize())?;
            let collator_options = cli.run.collator_options();

            runner.run_node_until_exit(|config| async move {
                let hwbench = if !cli.no_hardware_benchmarks {
                    config.database.path().map(|database_path| {
                        let _ = std::fs::create_dir_all(&database_path);
                        sc_sysinfo::gather_hwbench(Some(database_path))
                    })
                } else {
                    None
                };

                let para_id = Extensions::try_get(&*config.chain_spec)
                    .map(|e| e.para_id)
                    .ok_or_else(|| "Could not find parachain extension in chain-spec.")?;

                let polkadot_cli = RelayChainCli::new(
                    &config,
                    [RelayChainCli::executable_name().to_string()]
                        .iter()
                        .chain(cli.relaychain_args.iter()),
                );

                let id = ParaId::from(para_id);

                let parachain_account =
                    AccountIdConversion::<polkadot_primitives::AccountId>::into_account_truncating(
                        &id,
                    );

                let state_version =
                    RelayChainCli::native_runtime_version(&config.chain_spec).state_version();

                let block: crate::service::Block =
                    generate_genesis_block(&*config.chain_spec, state_version)
                        .map_err(|e| format!("{:?}", e))?;

                let genesis_state = format!("0x{:?}", HexDisplay::from(&block.header().encode()));

                let tokio_handle = config.tokio_handle.clone();
                let polkadot_config =
                    SubstrateCli::create_configuration(&polkadot_cli, &polkadot_cli, tokio_handle)
                        .map_err(|err| format!("Relay chain argument error: {}", err))?;

                info!("Parachain id: {:?}", id);
                info!("Parachain Account: {}", parachain_account);
                info!("Parachain genesis state: {}", genesis_state);
                info!(
                    "Is collating: {}",
                    if config.role.is_authority() {
                        "yes"
                    } else {
                        "no"
                    }
                );

                match &config.chain_spec {
                    #[cfg(feature = "with-eq-runtime")]
                    spec if spec.is_equilibrium() => {
                        service::start_node::<
                            eq_node_runtime::RuntimeApi,
                            service::EquilibriumRuntimeExecutor,
                        >(
                            config, polkadot_config, collator_options, id, hwbench
                        )
                        .await
                        .map(|r| r.0)
                        .map_err(Into::into)
                    }
                    #[cfg(feature = "with-gens-runtime")]
                    spec if spec.is_genshiro() => {
                        service::start_node::<
                            gens_node_runtime::RuntimeApi,
                            service::GenshiroRuntimeExecutor,
                        >(
                            config, polkadot_config, collator_options, id, hwbench
                        )
                        .await
                        .map(|r| r.0)
                        .map_err(Into::into)
                    }
                    _ => panic!("invalid chain spec"),
                }
            })
        }
    }
}

impl DefaultConfigurationValues for RelayChainCli {
    fn p2p_listen_port() -> u16 {
        30334
    }

    fn rpc_ws_listen_port() -> u16 {
        9945
    }

    fn rpc_http_listen_port() -> u16 {
        9934
    }

    fn prometheus_listen_port() -> u16 {
        9616
    }
}

impl CliConfiguration<Self> for RelayChainCli {
    fn shared_params(&self) -> &SharedParams {
        self.base.base.shared_params()
    }

    fn import_params(&self) -> Option<&ImportParams> {
        self.base.base.import_params()
    }

    fn network_params(&self) -> Option<&NetworkParams> {
        self.base.base.network_params()
    }

    fn keystore_params(&self) -> Option<&KeystoreParams> {
        self.base.base.keystore_params()
    }

    fn base_path(&self) -> Result<Option<BasePath>> {
        self.shared_params()
            .base_path()
            .or_else(|_| Ok(self.base_path.clone().map(Into::into)))
    }

    fn rpc_http(&self, default_listen_port: u16) -> Result<Option<SocketAddr>> {
        self.base.base.rpc_http(default_listen_port)
    }

    fn rpc_ipc(&self) -> Result<Option<String>> {
        self.base.base.rpc_ipc()
    }

    fn rpc_ws(&self, default_listen_port: u16) -> Result<Option<SocketAddr>> {
        self.base.base.rpc_ws(default_listen_port)
    }

    fn prometheus_config(
        &self,
        default_listen_port: u16,
        chain_spec: &Box<dyn ChainSpec>,
    ) -> Result<Option<PrometheusConfig>> {
        self.base
            .base
            .prometheus_config(default_listen_port, chain_spec)
    }

    fn init<F>(
        &self,
        _support_url: &String,
        _impl_version: &String,
        _logger_hook: F,
        _config: &sc_service::Configuration,
    ) -> Result<()>
    where
        F: FnOnce(&mut sc_cli::LoggerBuilder, &sc_service::Configuration),
    {
        unreachable!("PolkadotCli is never initialized; qed");
    }

    fn chain_id(&self, is_dev: bool) -> Result<String> {
        let chain_id = self.base.base.chain_id(is_dev)?;

        Ok(if chain_id.is_empty() {
            self.chain_id.clone().unwrap_or_default()
        } else {
            chain_id
        })
    }

    fn role(&self, is_dev: bool) -> Result<sc_service::Role> {
        self.base.base.role(is_dev)
    }

    fn transaction_pool(&self, is_dev: bool) -> Result<sc_service::config::TransactionPoolOptions> {
        self.base.base.transaction_pool(is_dev)
    }

    fn rpc_methods(&self) -> Result<sc_service::config::RpcMethods> {
        self.base.base.rpc_methods()
    }

    fn rpc_ws_max_connections(&self) -> Result<Option<usize>> {
        self.base.base.rpc_ws_max_connections()
    }

    fn rpc_cors(&self, is_dev: bool) -> Result<Option<Vec<String>>> {
        self.base.base.rpc_cors(is_dev)
    }

    fn default_heap_pages(&self) -> Result<Option<u64>> {
        self.base.base.default_heap_pages()
    }

    fn force_authoring(&self) -> Result<bool> {
        self.base.base.force_authoring()
    }

    fn disable_grandpa(&self) -> Result<bool> {
        self.base.base.disable_grandpa()
    }

    fn max_runtime_instances(&self) -> Result<Option<usize>> {
        self.base.base.max_runtime_instances()
    }

    fn announce_block(&self) -> Result<bool> {
        self.base.base.announce_block()
    }

    fn telemetry_endpoints(
        &self,
        chain_spec: &Box<dyn ChainSpec>,
    ) -> Result<Option<sc_telemetry::TelemetryEndpoints>> {
        self.base.base.telemetry_endpoints(chain_spec)
    }
}
