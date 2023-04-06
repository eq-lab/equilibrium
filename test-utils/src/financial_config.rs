///! Financial configs for tests and benchmarks
use core::str::FromStr;
use core::time::Duration;
use eq_primitives::{asset, asset::Asset};
use financial_pallet::{AssetMetrics, FinancialMetrics};
use sp_std::vec;
use sp_std::vec::Vec;
use substrate_fixed::types::I64F64;

pub fn get_per_asset_metrics(asset: Asset, period_start: Duration) -> AssetMetrics<Asset, I64F64> {
    let period_end: financial_pallet::Duration =
        Duration::from_secs(period_start.as_secs() + 48 * 60).into();
    let period_start: financial_pallet::Duration = period_start.into();
    // correlations should be sorted by asset!
    // ON_ADD_ASSET
    // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
    match asset {
        asset::ACA => {
            let mut correlations = vec![
                (asset::ACA, I64F64::from_str("1").unwrap()),
                (asset::BNB, I64F64::from_str("0.48596").unwrap()),
                (asset::BTC, I64F64::from_str("0.546287").unwrap()),
                (asset::CRV, I64F64::from_str("0.422242").unwrap()),
                (asset::DOT, I64F64::from_str("0.524914").unwrap()),
                (asset::EOS, I64F64::from_str("0.281569").unwrap()),
                (asset::ETH, I64F64::from_str("0.507273").unwrap()),
                (asset::AUSD, I64F64::from_str("0.461544").unwrap()),
                (asset::BUSD, I64F64::from_str("0.017031").unwrap()),
                (asset::GENS, I64F64::from_str("0.426502").unwrap()),
                (asset::GLMR, I64F64::from_str("0.366711").unwrap()),
                (asset::USDC, I64F64::from_str("-0.078121").unwrap()),
                (asset::EQ, I64F64::from_str("0.524914").unwrap()),
                (asset::HDOT, I64F64::from_str("0.524914").unwrap()),
                (asset::XDOT, I64F64::from_str("0.524914").unwrap()),
                (asset::EQD, I64F64::from_str("0").unwrap()),
                (asset::WBTC, I64F64::from_str("0.546287").unwrap()),
                (asset::DAI, I64F64::from_str("-0.072346").unwrap()),
                (asset::USDT, I64F64::from_str("-0.075574").unwrap()),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start,
                period_end,
                volatility: I64F64::from_str("0.045953").unwrap(),
                returns: vec![],
                correlations,
            }
        }
        asset::BNB => {
            let mut correlations = vec![
                (asset::ACA, I64F64::from_str("0.48596").unwrap()),
                (asset::BNB, I64F64::from_str("1").unwrap()),
                (asset::BTC, I64F64::from_str("0.562651").unwrap()),
                (asset::CRV, I64F64::from_str("0.274975").unwrap()),
                (asset::DOT, I64F64::from_str("0.570391").unwrap()),
                (asset::EOS, I64F64::from_str("0.311258").unwrap()),
                (asset::ETH, I64F64::from_str("0.474782").unwrap()),
                (asset::AUSD, I64F64::from_str("0.32203").unwrap()),
                (asset::BUSD, I64F64::from_str("0.038958").unwrap()),
                (asset::GENS, I64F64::from_str("0.413655").unwrap()),
                (asset::GLMR, I64F64::from_str("0.515929").unwrap()),
                (asset::USDC, I64F64::from_str("-0.15754").unwrap()),
                (asset::EQ, I64F64::from_str("0.570391").unwrap()),
                (asset::HDOT, I64F64::from_str("0.570391").unwrap()),
                (asset::XDOT, I64F64::from_str("0.570391").unwrap()),
                (asset::EQD, I64F64::from_str("0").unwrap()),
                (asset::WBTC, I64F64::from_str("0.562651").unwrap()),
                (asset::DAI, I64F64::from_str("-0.042208").unwrap()),
                (asset::USDT, I64F64::from_str("-0.009158").unwrap()),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start,
                period_end,
                volatility: I64F64::from_str("0.026968").unwrap(),
                returns: vec![],
                correlations,
            }
        }
        asset::BTC => {
            let mut correlations = vec![
                (asset::ACA, I64F64::from_str("0.546287").unwrap()),
                (asset::BNB, I64F64::from_str("0.562651").unwrap()),
                (asset::BTC, I64F64::from_str("1").unwrap()),
                (asset::CRV, I64F64::from_str("0.56261").unwrap()),
                (asset::DOT, I64F64::from_str("0.743001").unwrap()),
                (asset::EOS, I64F64::from_str("0.59912").unwrap()),
                (asset::ETH, I64F64::from_str("0.911058").unwrap()),
                (asset::AUSD, I64F64::from_str("0.240666").unwrap()),
                (asset::BUSD, I64F64::from_str("0.05027").unwrap()),
                (asset::GENS, I64F64::from_str("0.645233").unwrap()),
                (asset::GLMR, I64F64::from_str("0.493756").unwrap()),
                (asset::USDC, I64F64::from_str("0.064253").unwrap()),
                (asset::EQ, I64F64::from_str("0.743001").unwrap()),
                (asset::HDOT, I64F64::from_str("0.743001").unwrap()),
                (asset::XDOT, I64F64::from_str("0.743001").unwrap()),
                (asset::EQD, I64F64::from_str("0").unwrap()),
                (asset::WBTC, I64F64::from_str("1").unwrap()),
                (asset::DAI, I64F64::from_str("0.211378").unwrap()),
                (asset::USDT, I64F64::from_str("-0.067568").unwrap()),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start,
                period_end,
                volatility: I64F64::from_str("0.024482").unwrap(),
                returns: vec![],
                correlations,
            }
        }
        asset::CRV => {
            let mut correlations = vec![
                // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                (asset::ACA, I64F64::from_str("0.422242").unwrap()),
                (asset::BNB, I64F64::from_str("0.274975").unwrap()),
                (asset::BTC, I64F64::from_str("0.56261").unwrap()),
                (asset::CRV, I64F64::from_str("1").unwrap()),
                (asset::DOT, I64F64::from_str("0.5859").unwrap()),
                (asset::EOS, I64F64::from_str("0.358865").unwrap()),
                (asset::ETH, I64F64::from_str("0.689762").unwrap()),
                (asset::AUSD, I64F64::from_str("0.064257").unwrap()),
                (asset::BUSD, I64F64::from_str("-0.228226").unwrap()),
                (asset::GENS, I64F64::from_str("0.233073").unwrap()),
                (asset::GLMR, I64F64::from_str("0.406619").unwrap()),
                (asset::USDC, I64F64::from_str("-0.166676").unwrap()),
                (asset::EQ, I64F64::from_str("0.5859").unwrap()),
                (asset::HDOT, I64F64::from_str("0.5859").unwrap()),
                (asset::XDOT, I64F64::from_str("0.5859").unwrap()),
                (asset::EQD, I64F64::from_str("0").unwrap()),
                (asset::WBTC, I64F64::from_str("0.56261").unwrap()),
                (asset::DAI, I64F64::from_str("0.130464").unwrap()),
                (asset::USDT, I64F64::from_str("0.173315").unwrap()),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start,
                period_end,
                volatility: I64F64::from_str("0.047755").unwrap(),
                returns: vec![],
                correlations,
            }
        }
        asset::DOT | asset::HDOT | asset::XDOT => {
            let mut correlations = vec![
                (asset::ACA, I64F64::from_str("0.524914").unwrap()),
                (asset::BNB, I64F64::from_str("0.570391").unwrap()),
                (asset::BTC, I64F64::from_str("0.743001").unwrap()),
                (asset::CRV, I64F64::from_str("0.5859").unwrap()),
                (asset::DOT, I64F64::from_str("1").unwrap()),
                (asset::EOS, I64F64::from_str("0.485283").unwrap()),
                (asset::ETH, I64F64::from_str("0.80495").unwrap()),
                (asset::AUSD, I64F64::from_str("0.292728").unwrap()),
                (asset::BUSD, I64F64::from_str("-0.042013").unwrap()),
                (asset::GENS, I64F64::from_str("0.420751").unwrap()),
                (asset::GLMR, I64F64::from_str("0.718004").unwrap()),
                (asset::USDC, I64F64::from_str("-0.259564").unwrap()),
                (asset::EQ, I64F64::from_str("1").unwrap()),
                (asset::HDOT, I64F64::from_str("1").unwrap()),
                (asset::XDOT, I64F64::from_str("1").unwrap()),
                (asset::EQD, I64F64::from_str("0").unwrap()),
                (asset::WBTC, I64F64::from_str("0.743001").unwrap()),
                (asset::DAI, I64F64::from_str("0.112336").unwrap()),
                (asset::USDT, I64F64::from_str("0.093847").unwrap()),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start,
                period_end,
                volatility: I64F64::from_str("0.040859").unwrap(),
                returns: vec![],
                correlations,
            }
        }
        asset::EOS => {
            let mut correlations = vec![
                (asset::ACA, I64F64::from_str("0.281569").unwrap()),
                (asset::BNB, I64F64::from_str("0.311258").unwrap()),
                (asset::BTC, I64F64::from_str("0.59912").unwrap()),
                (asset::CRV, I64F64::from_str("0.358865").unwrap()),
                (asset::DOT, I64F64::from_str("0.485283").unwrap()),
                (asset::EOS, I64F64::from_str("1").unwrap()),
                (asset::ETH, I64F64::from_str("0.566662").unwrap()),
                (asset::AUSD, I64F64::from_str("0.113015").unwrap()),
                (asset::BUSD, I64F64::from_str("-0.049987").unwrap()),
                (asset::GENS, I64F64::from_str("0.430452").unwrap()),
                (asset::GLMR, I64F64::from_str("0.426984").unwrap()),
                (asset::USDC, I64F64::from_str("0.191446").unwrap()),
                (asset::EQ, I64F64::from_str("0.485283").unwrap()),
                (asset::HDOT, I64F64::from_str("0.485283").unwrap()),
                (asset::XDOT, I64F64::from_str("0.485283").unwrap()),
                (asset::EQD, I64F64::from_str("0").unwrap()),
                (asset::WBTC, I64F64::from_str("0.59912").unwrap()),
                (asset::DAI, I64F64::from_str("-0.040165").unwrap()),
                (asset::USDT, I64F64::from_str("-0.03998").unwrap()),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start,
                period_end,
                volatility: I64F64::from_str("0.060373").unwrap(),
                returns: vec![],
                correlations,
            }
        }
        asset::ETH => {
            let mut correlations = vec![
                (asset::ACA, I64F64::from_str("0.507273").unwrap()),
                (asset::BNB, I64F64::from_str("0.474782").unwrap()),
                (asset::BTC, I64F64::from_str("0.911058").unwrap()),
                (asset::CRV, I64F64::from_str("0.689762").unwrap()),
                (asset::DOT, I64F64::from_str("0.80495").unwrap()),
                (asset::EOS, I64F64::from_str("0.566662").unwrap()),
                (asset::ETH, I64F64::from_str("1").unwrap()),
                (asset::AUSD, I64F64::from_str("0.217292").unwrap()),
                (asset::BUSD, I64F64::from_str("-0.065267").unwrap()),
                (asset::GENS, I64F64::from_str("0.554247").unwrap()),
                (asset::GLMR, I64F64::from_str("0.567505").unwrap()),
                (asset::USDC, I64F64::from_str("-0.006955").unwrap()),
                (asset::EQ, I64F64::from_str("0.80495").unwrap()),
                (asset::HDOT, I64F64::from_str("0.80495").unwrap()),
                (asset::XDOT, I64F64::from_str("0.80495").unwrap()),
                (asset::EQD, I64F64::from_str("0").unwrap()),
                (asset::WBTC, I64F64::from_str("0.911058").unwrap()),
                (asset::DAI, I64F64::from_str("0.132768").unwrap()),
                (asset::USDT, I64F64::from_str("0.03119").unwrap()),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start,
                period_end,
                volatility: I64F64::from_str("0.039117").unwrap(),
                returns: vec![],
                correlations,
            }
        }
        asset::AUSD => {
            let mut correlations = vec![
                (asset::ACA, I64F64::from_str("0.461544").unwrap()),
                (asset::BNB, I64F64::from_str("0.32203").unwrap()),
                (asset::BTC, I64F64::from_str("0.240666").unwrap()),
                (asset::CRV, I64F64::from_str("0.064257").unwrap()),
                (asset::DOT, I64F64::from_str("0.292728").unwrap()),
                (asset::EOS, I64F64::from_str("0.113015").unwrap()),
                (asset::ETH, I64F64::from_str("0.217292").unwrap()),
                (asset::AUSD, I64F64::from_str("1").unwrap()),
                (asset::BUSD, I64F64::from_str("0.173504").unwrap()),
                (asset::GENS, I64F64::from_str("0.004999").unwrap()),
                (asset::GLMR, I64F64::from_str("0.360772").unwrap()),
                (asset::USDC, I64F64::from_str("-0.004127").unwrap()),
                (asset::EQ, I64F64::from_str("0.292728").unwrap()),
                (asset::HDOT, I64F64::from_str("0.292728").unwrap()),
                (asset::XDOT, I64F64::from_str("0.292728").unwrap()),
                (asset::EQD, I64F64::from_str("0").unwrap()),
                (asset::WBTC, I64F64::from_str("0.240666").unwrap()),
                (asset::DAI, I64F64::from_str("-0.329046").unwrap()),
                (asset::USDT, I64F64::from_str("-0.074972").unwrap()),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start,
                period_end,
                volatility: I64F64::from_str("0.050826").unwrap(),
                returns: vec![],
                correlations,
            }
        }
        asset::BUSD => {
            let mut correlations = vec![
                (asset::ACA, I64F64::from_str("0.017031").unwrap()),
                (asset::BNB, I64F64::from_str("0.038958").unwrap()),
                (asset::BTC, I64F64::from_str("0.05027").unwrap()),
                (asset::CRV, I64F64::from_str("-0.228226").unwrap()),
                (asset::DOT, I64F64::from_str("-0.042013").unwrap()),
                (asset::EOS, I64F64::from_str("-0.049987").unwrap()),
                (asset::ETH, I64F64::from_str("-0.065267").unwrap()),
                (asset::AUSD, I64F64::from_str("0.173504").unwrap()),
                (asset::BUSD, I64F64::from_str("1").unwrap()),
                (asset::GENS, I64F64::from_str("0.03177").unwrap()),
                (asset::GLMR, I64F64::from_str("-0.356066").unwrap()),
                (asset::USDC, I64F64::from_str("0.208696").unwrap()),
                (asset::EQ, I64F64::from_str("-0.042013").unwrap()),
                (asset::HDOT, I64F64::from_str("-0.042013").unwrap()),
                (asset::XDOT, I64F64::from_str("-0.042013").unwrap()),
                (asset::EQD, I64F64::from_str("0").unwrap()),
                (asset::WBTC, I64F64::from_str("0.05027").unwrap()),
                (asset::DAI, I64F64::from_str("-0.012399").unwrap()),
                (asset::USDT, I64F64::from_str("-0.059853").unwrap()),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start,
                period_end,
                volatility: I64F64::from_str("0.000576").unwrap(),
                returns: vec![],
                correlations,
            }
        }
        asset::GENS => {
            let mut correlations = vec![
                (asset::ACA, I64F64::from_str("0.426502").unwrap()),
                (asset::BNB, I64F64::from_str("0.413655").unwrap()),
                (asset::BTC, I64F64::from_str("0.645233").unwrap()),
                (asset::CRV, I64F64::from_str("0.233073").unwrap()),
                (asset::DOT, I64F64::from_str("0.420751").unwrap()),
                (asset::EOS, I64F64::from_str("0.430452").unwrap()),
                (asset::ETH, I64F64::from_str("0.554247").unwrap()),
                (asset::AUSD, I64F64::from_str("0.004999").unwrap()),
                (asset::BUSD, I64F64::from_str("0.03177").unwrap()),
                (asset::GENS, I64F64::from_str("1").unwrap()),
                (asset::GLMR, I64F64::from_str("0.236236").unwrap()),
                (asset::USDC, I64F64::from_str("0.046375").unwrap()),
                (asset::EQ, I64F64::from_str("0.420751").unwrap()),
                (asset::HDOT, I64F64::from_str("0.420751").unwrap()),
                (asset::XDOT, I64F64::from_str("0.420751").unwrap()),
                (asset::EQD, I64F64::from_str("0").unwrap()),
                (asset::WBTC, I64F64::from_str("0.645233").unwrap()),
                (asset::DAI, I64F64::from_str("-0.070477").unwrap()),
                (asset::USDT, I64F64::from_str("-0.154645").unwrap()),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start,
                period_end,
                volatility: I64F64::from_str("0.045523").unwrap(),
                returns: vec![],
                correlations,
            }
        }
        asset::GLMR => {
            let mut correlations = vec![
                (asset::ACA, I64F64::from_str("0.366711").unwrap()),
                (asset::BNB, I64F64::from_str("0.515929").unwrap()),
                (asset::BTC, I64F64::from_str("0.493756").unwrap()),
                (asset::CRV, I64F64::from_str("0.406619").unwrap()),
                (asset::DOT, I64F64::from_str("0.718004").unwrap()),
                (asset::EOS, I64F64::from_str("0.426984").unwrap()),
                (asset::ETH, I64F64::from_str("0.567505").unwrap()),
                (asset::AUSD, I64F64::from_str("0.360772").unwrap()),
                (asset::BUSD, I64F64::from_str("-0.356066").unwrap()),
                (asset::GENS, I64F64::from_str("0.236236").unwrap()),
                (asset::GLMR, I64F64::from_str("1").unwrap()),
                (asset::USDC, I64F64::from_str("-0.184183").unwrap()),
                (asset::EQ, I64F64::from_str("0.718004").unwrap()),
                (asset::HDOT, I64F64::from_str("0.718004").unwrap()),
                (asset::XDOT, I64F64::from_str("0.718004").unwrap()),
                (asset::EQD, I64F64::from_str("0").unwrap()),
                (asset::WBTC, I64F64::from_str("0.493756").unwrap()),
                (asset::DAI, I64F64::from_str("0.13062").unwrap()),
                (asset::USDT, I64F64::from_str("0.073426").unwrap()),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start,
                period_end,
                volatility: I64F64::from_str("0.038604").unwrap(),
                returns: vec![],
                correlations,
            }
        }
        asset::USDC => {
            let mut correlations = vec![
                (asset::ACA, I64F64::from_str("-0.078121").unwrap()),
                (asset::BNB, I64F64::from_str("-0.15754").unwrap()),
                (asset::BTC, I64F64::from_str("0.064253").unwrap()),
                (asset::CRV, I64F64::from_str("-0.166676").unwrap()),
                (asset::DOT, I64F64::from_str("-0.259564").unwrap()),
                (asset::EOS, I64F64::from_str("0.191446").unwrap()),
                (asset::ETH, I64F64::from_str("-0.006955").unwrap()),
                (asset::AUSD, I64F64::from_str("-0.004127").unwrap()),
                (asset::BUSD, I64F64::from_str("0.208696").unwrap()),
                (asset::GENS, I64F64::from_str("0.046375").unwrap()),
                (asset::GLMR, I64F64::from_str("-0.184183").unwrap()),
                (asset::USDC, I64F64::from_str("1").unwrap()),
                (asset::EQ, I64F64::from_str("-0.259564").unwrap()),
                (asset::HDOT, I64F64::from_str("-0.259564").unwrap()),
                (asset::XDOT, I64F64::from_str("-0.259564").unwrap()),
                (asset::EQD, I64F64::from_str("0").unwrap()),
                (asset::WBTC, I64F64::from_str("0.064253").unwrap()),
                (asset::DAI, I64F64::from_str("0.158475").unwrap()),
                (asset::USDT, I64F64::from_str("-0.378121").unwrap()),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start,
                period_end,
                volatility: I64F64::from_str("0.000096").unwrap(),
                returns: vec![],
                correlations,
            }
        }
        asset::EQ => {
            let mut correlations = vec![
                (asset::ACA, I64F64::from_str("0.524914").unwrap()),
                (asset::BNB, I64F64::from_str("0.570391").unwrap()),
                (asset::BTC, I64F64::from_str("0.743001").unwrap()),
                (asset::CRV, I64F64::from_str("0.5859").unwrap()),
                (asset::DOT, I64F64::from_str("1").unwrap()),
                (asset::EOS, I64F64::from_str("0.485283").unwrap()),
                (asset::ETH, I64F64::from_str("0.80495").unwrap()),
                (asset::AUSD, I64F64::from_str("0.292728").unwrap()),
                (asset::BUSD, I64F64::from_str("-0.042013").unwrap()),
                (asset::GENS, I64F64::from_str("0.420751").unwrap()),
                (asset::GLMR, I64F64::from_str("0.718004").unwrap()),
                (asset::USDC, I64F64::from_str("-0.259564").unwrap()),
                (asset::EQ, I64F64::from_str("1").unwrap()),
                (asset::HDOT, I64F64::from_str("1").unwrap()),
                (asset::XDOT, I64F64::from_str("1").unwrap()),
                (asset::EQD, I64F64::from_str("0").unwrap()),
                (asset::WBTC, I64F64::from_str("0.743001").unwrap()),
                (asset::DAI, I64F64::from_str("0.112336").unwrap()),
                (asset::USDT, I64F64::from_str("0.093847").unwrap()),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start,
                period_end,
                volatility: I64F64::from_str("0.040859").unwrap(),
                returns: vec![],
                correlations,
            }
        }
        asset::EQD => {
            let mut correlations = vec![
                (asset::ACA, I64F64::from_str("0").unwrap()),
                (asset::BNB, I64F64::from_str("0").unwrap()),
                (asset::BTC, I64F64::from_str("0").unwrap()),
                (asset::CRV, I64F64::from_str("0").unwrap()),
                (asset::DOT, I64F64::from_str("0").unwrap()),
                (asset::EOS, I64F64::from_str("0").unwrap()),
                (asset::ETH, I64F64::from_str("0").unwrap()),
                (asset::AUSD, I64F64::from_str("0").unwrap()),
                (asset::BUSD, I64F64::from_str("0").unwrap()),
                (asset::GENS, I64F64::from_str("0").unwrap()),
                (asset::GLMR, I64F64::from_str("0").unwrap()),
                (asset::USDC, I64F64::from_str("0").unwrap()),
                (asset::EQ, I64F64::from_str("0").unwrap()),
                (asset::HDOT, I64F64::from_str("0").unwrap()),
                (asset::XDOT, I64F64::from_str("0").unwrap()),
                (asset::EQD, I64F64::from_str("0").unwrap()),
                (asset::WBTC, I64F64::from_str("0").unwrap()),
                (asset::DAI, I64F64::from_str("0").unwrap()),
                (asset::USDT, I64F64::from_str("0").unwrap()),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start,
                period_end,
                volatility: I64F64::from_str("0").unwrap(),
                returns: vec![],
                correlations,
            }
        }
        asset::WBTC => {
            let mut correlations = vec![
                (asset::ACA, I64F64::from_str("0.546287").unwrap()),
                (asset::BNB, I64F64::from_str("0.562651").unwrap()),
                (asset::BTC, I64F64::from_str("1").unwrap()),
                (asset::CRV, I64F64::from_str("0.56261").unwrap()),
                (asset::DOT, I64F64::from_str("0.743001").unwrap()),
                (asset::EOS, I64F64::from_str("0.59912").unwrap()),
                (asset::ETH, I64F64::from_str("0.911058").unwrap()),
                (asset::AUSD, I64F64::from_str("0.240666").unwrap()),
                (asset::BUSD, I64F64::from_str("0.05027").unwrap()),
                (asset::GENS, I64F64::from_str("0.645233").unwrap()),
                (asset::GLMR, I64F64::from_str("0.493756").unwrap()),
                (asset::USDC, I64F64::from_str("0.064253").unwrap()),
                (asset::EQ, I64F64::from_str("0.743001").unwrap()),
                (asset::HDOT, I64F64::from_str("0.743001").unwrap()),
                (asset::XDOT, I64F64::from_str("0.743001").unwrap()),
                (asset::EQD, I64F64::from_str("0").unwrap()),
                (asset::WBTC, I64F64::from_str("1").unwrap()),
                (asset::DAI, I64F64::from_str("0.211378").unwrap()),
                (asset::USDT, I64F64::from_str("-0.067568").unwrap()),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start,
                period_end,
                volatility: I64F64::from_str("0.024482").unwrap(),
                returns: vec![],
                correlations,
            }
        }
        asset::DAI => {
            let mut correlations = vec![
                (asset::ACA, I64F64::from_str("-0.072346").unwrap()),
                (asset::BNB, I64F64::from_str("-0.042208").unwrap()),
                (asset::BTC, I64F64::from_str("0.211378").unwrap()),
                (asset::CRV, I64F64::from_str("0.130464").unwrap()),
                (asset::DOT, I64F64::from_str("0.112336").unwrap()),
                (asset::EOS, I64F64::from_str("-0.040165").unwrap()),
                (asset::ETH, I64F64::from_str("0.132768").unwrap()),
                (asset::AUSD, I64F64::from_str("-0.329046").unwrap()),
                (asset::BUSD, I64F64::from_str("-0.012399").unwrap()),
                (asset::GENS, I64F64::from_str("-0.070477").unwrap()),
                (asset::GLMR, I64F64::from_str("0.13062").unwrap()),
                (asset::USDC, I64F64::from_str("0.158475").unwrap()),
                (asset::EQ, I64F64::from_str("0.112336").unwrap()),
                (asset::HDOT, I64F64::from_str("0.112336").unwrap()),
                (asset::XDOT, I64F64::from_str("0.112336").unwrap()),
                (asset::EQD, I64F64::from_str("0").unwrap()),
                (asset::WBTC, I64F64::from_str("0.211378").unwrap()),
                (asset::DAI, I64F64::from_str("1").unwrap()),
                (asset::USDT, I64F64::from_str("-0.379623").unwrap()),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start,
                period_end,
                volatility: I64F64::from_str("0.000206").unwrap(),
                returns: vec![],
                correlations,
            }
        }
        asset::USDT => {
            let mut correlations = vec![
                (asset::ACA, I64F64::from_str("-0.075574").unwrap()),
                (asset::BNB, I64F64::from_str("-0.009158").unwrap()),
                (asset::BTC, I64F64::from_str("-0.067568").unwrap()),
                (asset::CRV, I64F64::from_str("0.173315").unwrap()),
                (asset::DOT, I64F64::from_str("0.093847").unwrap()),
                (asset::EOS, I64F64::from_str("-0.03998").unwrap()),
                (asset::ETH, I64F64::from_str("0.03119").unwrap()),
                (asset::AUSD, I64F64::from_str("-0.074972").unwrap()),
                (asset::BUSD, I64F64::from_str("-0.059853").unwrap()),
                (asset::GENS, I64F64::from_str("-0.154645").unwrap()),
                (asset::GLMR, I64F64::from_str("0.073426").unwrap()),
                (asset::USDC, I64F64::from_str("-0.378121").unwrap()),
                (asset::EQ, I64F64::from_str("0.093847").unwrap()),
                (asset::HDOT, I64F64::from_str("0.093847").unwrap()),
                (asset::XDOT, I64F64::from_str("0.093847").unwrap()),
                (asset::EQD, I64F64::from_str("0").unwrap()),
                (asset::WBTC, I64F64::from_str("-0.067568").unwrap()),
                (asset::DAI, I64F64::from_str("-0.379623").unwrap()),
                (asset::USDT, I64F64::from_str("1").unwrap()),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start,
                period_end,
                volatility: I64F64::from_str("0.000093").unwrap(),
                returns: vec![],
                correlations,
            }
        }
        asset::PARA => {
            let mut correlations = vec![
                // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                (asset::EQ, I64F64::from_num(0)),
                (asset::BNB, I64F64::from_num(0)),
                (asset::BTC, I64F64::from_num(0)),
                (asset::WBTC, I64F64::from_num(0)),
                (asset::CRV, I64F64::from_num(0)),
                (asset::DAI, I64F64::from_num(0)),
                (asset::DOT, I64F64::from_num(0)),
                (asset::HDOT, I64F64::from_num(0)),
                (asset::XDOT, I64F64::from_num(0)),
                (asset::EOS, I64F64::from_num(0)),
                (asset::ETH, I64F64::from_num(0)),
                (asset::EQD, I64F64::from_num(0)),
                (asset::BUSD, I64F64::from_num(0)),
                (asset::GENS, I64F64::from_num(0)),
                (asset::USDC, I64F64::from_num(0)),
                (asset::USDT, I64F64::from_num(0)),
                (asset::ACA, I64F64::from_num(0)),
                (asset::AUSD, I64F64::from_num(0)),
                (asset::GLMR, I64F64::from_num(0)),
                (asset::PARA, I64F64::from_num(1)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start,
                period_end,
                volatility: I64F64::from_num(0),
                returns: vec![],
                correlations,
            }
        }
        asset::EQDOT => {
            let mut correlations = vec![
                // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                (asset::EQ, I64F64::from_num(0)),
                (asset::BNB, I64F64::from_num(0)),
                (asset::BTC, I64F64::from_num(0)),
                (asset::WBTC, I64F64::from_num(0)),
                (asset::CRV, I64F64::from_num(0)),
                (asset::DAI, I64F64::from_num(0)),
                (asset::DOT, I64F64::from_num(0)),
                (asset::HDOT, I64F64::from_num(0)),
                (asset::XDOT, I64F64::from_num(0)),
                (asset::EOS, I64F64::from_num(0)),
                (asset::ETH, I64F64::from_num(0)),
                (asset::EQD, I64F64::from_num(0)),
                (asset::BUSD, I64F64::from_num(0)),
                (asset::GENS, I64F64::from_num(0)),
                (asset::USDC, I64F64::from_num(0)),
                (asset::USDT, I64F64::from_num(0)),
                (asset::ACA, I64F64::from_num(0)),
                (asset::AUSD, I64F64::from_num(0)),
                (asset::GLMR, I64F64::from_num(0)),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(1)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start,
                period_end,
                volatility: I64F64::from_num(0),
                returns: vec![],
                correlations,
            }
        }
        a => {
            panic!("Exhausted options. Asset: {:?}", a)
        }
    }
}

pub fn get_metrics(period_start: Duration) -> FinancialMetrics<Asset, I64F64> {
    let period_end = Duration::from_secs(period_start.as_secs() + 48 * 60);
    let mut assets = vec![
        // ON_ADD_ASSET
        asset::EQ,
        asset::BNB,
        asset::BTC,
        asset::WBTC,
        asset::CRV,
        asset::DAI,
        asset::DOT,
        asset::HDOT,
        asset::XDOT,
        asset::EOS,
        asset::ETH,
        asset::EQD,
        asset::BUSD,
        asset::GENS,
        asset::USDC,
        asset::USDT,
        asset::ACA,
        asset::AUSD,
        asset::GLMR,
        asset::PARA,
        asset::EQDOT,
    ];
    assets.sort_by(|a1, a2| a1.cmp(a2));
    let volatilities: Vec<_> = assets
        .iter()
        .map(|a| get_per_asset_metrics(*a, period_start).volatility)
        .collect();

    assert_eq!(assets.len(), volatilities.len());

    let correlations: Vec<_> = assets
        .iter()
        .map(|a| {
            get_per_asset_metrics(*a, period_start)
                .correlations
                .iter()
                .map(|(_, c)| *c)
                .collect::<Vec<I64F64>>()
        })
        .flatten()
        .collect();

    assert_eq!(assets.len() * assets.len(), correlations.len());

    let covariances: Vec<_> = (0..assets.len())
        .into_iter()
        .flat_map(|idx| {
            (0..assets.len())
                .into_iter()
                .map(|idy| {
                    correlations[idx * assets.len() + idy] * volatilities[idx] * volatilities[idy]
                })
                .collect::<Vec<_>>()
        })
        .collect();

    assert_eq!(assets.len() * assets.len(), covariances.len());

    FinancialMetrics {
        period_start: period_start.into(),
        period_end: period_end.into(),
        assets,
        mean_returns: vec![],
        volatilities,
        correlations,
        covariances, /*vec![
                         // ON_ADD_ASSET
                         // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                         // Eq asset covariances
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Eq
                         I64F64::from_num(0),                                          // Bnb
                         I64F64::from_num(11788778) / I64F64::from_num(1_000_000_000), // Btc
                         I64F64::from_num(0),                                          // Wbtc
                         I64F64::from_num(12345345) / I64F64::from_num(1_000_000_000), // Crv
                         I64F64::from_num(0),                                          // Dai
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Dot
                         I64F64::from_num(5824929) / I64F64::from_num(1_000_000_000),  // Eos
                         I64F64::from_num(15154913) / I64F64::from_num(1_000_000_000), // Eth
                         I64F64::from_num(0),                                          // Eqd
                         I64F64::from_num(0),                                          // Busd
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Gens
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Hdot
                         I64F64::from_num(0),                                          // Usdc
                         I64F64::from_num(0),                                          // Usdt
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Xdot
                         I64F64::from_num(0),                                          // Aca
                         I64F64::from_num(0),                                          // Ausd
                         I64F64::from_num(0),                                          // Glmr
                         I64F64::from_num(0),                                          // Para
                         // Bnb covariances
                         I64F64::from_num(0), // Eq
                         I64F64::from_num(0), // Bnb
                         I64F64::from_num(0), // Btc
                         I64F64::from_num(0), // Wbtc
                         I64F64::from_num(0), // Crv
                         I64F64::from_num(0), // Dai
                         I64F64::from_num(0), // Dot
                         I64F64::from_num(0), // Eos
                         I64F64::from_num(0), // Eth
                         I64F64::from_num(0), // Eqd
                         I64F64::from_num(0), // Busd
                         I64F64::from_num(0), // Gens
                         I64F64::from_num(0), // Hdot
                         I64F64::from_num(0), // Usdc
                         I64F64::from_num(0), // Usdt
                         I64F64::from_num(0), // Xdot
                         I64F64::from_num(0), // Aca
                         I64F64::from_num(0), // Ausd
                         I64F64::from_num(0), // Glmr
                         I64F64::from_num(0), // Para
                         // Btc covariances
                         I64F64::from_num(11788778) / I64F64::from_num(1_000_000_000), // Eq
                         I64F64::from_num(0),                                          // Bnb
                         I64F64::from_num(12307905) / I64F64::from_num(1_000_000_000), // Btc
                         I64F64::from_num(0),                                          // Wbtc
                         I64F64::from_num(12345678) / I64F64::from_num(1_000_000_000), // Crv
                         I64F64::from_num(0),                                          // Dai
                         I64F64::from_num(11788778) / I64F64::from_num(1_000_000_000), // Dot
                         I64F64::from_num(8142116) / I64F64::from_num(1_000_000_000),  // Eos
                         I64F64::from_num(13742562) / I64F64::from_num(1_000_000_000), // Eth
                         I64F64::from_num(0),                                          // Eqd
                         I64F64::from_num(0),                                          // Busd
                         I64F64::from_num(11788778) / I64F64::from_num(1_000_000_000), // Gens
                         I64F64::from_num(11788778) / I64F64::from_num(1_000_000_000), // Hdot
                         I64F64::from_num(0),                                          // Usdc
                         I64F64::from_num(0),                                          // Usdt
                         I64F64::from_num(11788778) / I64F64::from_num(1_000_000_000), // Xdot
                         I64F64::from_num(0),                                          // Aca
                         I64F64::from_num(0),                                          // Ausd
                         I64F64::from_num(0),                                          // Glmr
                         I64F64::from_num(0),                                          // Para
                         // Wbtc covariances
                         I64F64::from_num(0), // Eq
                         I64F64::from_num(0), // Bnb
                         I64F64::from_num(0), // Btc
                         I64F64::from_num(0), // Wbtc
                         I64F64::from_num(0), // Crv
                         I64F64::from_num(0), // Dai
                         I64F64::from_num(0), // Dot
                         I64F64::from_num(0), // Eos
                         I64F64::from_num(0), // Eth
                         I64F64::from_num(0), // Eqd
                         I64F64::from_num(0), // Busd
                         I64F64::from_num(0), // Gens
                         I64F64::from_num(0), // Hdot
                         I64F64::from_num(0), // Usdc
                         I64F64::from_num(0), // Usdt
                         I64F64::from_num(0), // Xdot
                         I64F64::from_num(0), // Aca
                         I64F64::from_num(0), // Ausd
                         I64F64::from_num(0), // Glmr
                         I64F64::from_num(0), // Para
                         // Crv covariances
                         I64F64::from_num(12345345) / I64F64::from_num(1_000_000_000), // Eq
                         I64F64::from_num(0),                                          // Bnb
                         I64F64::from_num(12345678) / I64F64::from_num(1_000_000_000), // Btc
                         I64F64::from_num(0),                                          // Wbtc
                         I64F64::from_num(23267912) / I64F64::from_num(1_000_000_000), // Crv
                         I64F64::from_num(0),                                          // Dai
                         I64F64::from_num(13445676) / I64F64::from_num(1_000_000_000), // Dot
                         I64F64::from_num(9283237) / I64F64::from_num(1_000_000_000),  // Eos
                         I64F64::from_num(14555672) / I64F64::from_num(1_000_000_000), // Eth
                         I64F64::from_num(0),                                          // Eqd
                         I64F64::from_num(0),                                          // Busd
                         I64F64::from_num(12345345) / I64F64::from_num(1_000_000_000), // Gens
                         I64F64::from_num(13445676) / I64F64::from_num(1_000_000_000), // Hdot
                         I64F64::from_num(0),                                          // Usdc
                         I64F64::from_num(0),                                          // Usdt
                         I64F64::from_num(13445676) / I64F64::from_num(1_000_000_000), // Xdot
                         I64F64::from_num(0),                                          // Aca
                         I64F64::from_num(0),                                          // Ausd
                         I64F64::from_num(0),                                          // Glmr
                         I64F64::from_num(0),                                          // Para
                         // Dai covariances
                         I64F64::from_num(0), // Eq
                         I64F64::from_num(0), // Bnb
                         I64F64::from_num(0), // Btc
                         I64F64::from_num(0), // Wbtc
                         I64F64::from_num(0), // Crv
                         I64F64::from_num(0), // Dai
                         I64F64::from_num(0), // Dot
                         I64F64::from_num(0), // Eos
                         I64F64::from_num(0), // Eth
                         I64F64::from_num(0), // Eqd
                         I64F64::from_num(0), // Busd
                         I64F64::from_num(0), // Gens
                         I64F64::from_num(0), // Hdot
                         I64F64::from_num(0), // Usdc
                         I64F64::from_num(0), // Usdt
                         I64F64::from_num(0), // Xdot
                         I64F64::from_num(0), // Aca
                         I64F64::from_num(0), // Ausd
                         I64F64::from_num(0), // Glmr
                         I64F64::from_num(0), // Para
                         // Dot covariances
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Eq
                         I64F64::from_num(0),                                          // Bnb
                         I64F64::from_num(11788778) / I64F64::from_num(1_000_000_000), // Btc
                         I64F64::from_num(0),                                          // Wbtc
                         I64F64::from_num(13445676) / I64F64::from_num(1_000_000_000), // Crv
                         I64F64::from_num(0),                                          // Dai
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Dot
                         I64F64::from_num(5824929) / I64F64::from_num(1_000_000_000),  // Eos
                         I64F64::from_num(15154913) / I64F64::from_num(1_000_000_000), // Eth
                         I64F64::from_num(0),                                          // Eqd
                         I64F64::from_num(0),                                          // Busd
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Gens
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Hdot
                         I64F64::from_num(0),                                          // Usdc
                         I64F64::from_num(0),                                          // Usdt
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Xdot
                         I64F64::from_num(0),                                          // Aca
                         I64F64::from_num(0),                                          // Ausd
                         I64F64::from_num(0),                                          // Glmr
                         I64F64::from_num(0),                                          // Para
                         // Eos covariances
                         I64F64::from_num(5824929) / I64F64::from_num(1_000_000_000), // Eq
                         I64F64::from_num(0),                                         // Bnb
                         I64F64::from_num(8142116) / I64F64::from_num(1_000_000_000), // Btc
                         I64F64::from_num(0),                                         // Wbtc
                         I64F64::from_num(9283237) / I64F64::from_num(1_000_000_000), // Crv
                         I64F64::from_num(0),                                         // Dai
                         I64F64::from_num(5824929) / I64F64::from_num(1_000_000_000), // Dot
                         I64F64::from_num(17743306) / I64F64::from_num(1_000_000_000), // Eos
                         I64F64::from_num(10413835) / I64F64::from_num(1_000_000_000), // Eth
                         I64F64::from_num(0),                                         // Eqd
                         I64F64::from_num(0),                                         // Busd
                         I64F64::from_num(5824929) / I64F64::from_num(1_000_000_000), // Gens
                         I64F64::from_num(5824929) / I64F64::from_num(1_000_000_000), // Hdot
                         I64F64::from_num(0),                                         // Usdc
                         I64F64::from_num(0),                                         // Usdt
                         I64F64::from_num(5824929) / I64F64::from_num(1_000_000_000), // Xdot
                         I64F64::from_num(0),                                         // Aca
                         I64F64::from_num(0),                                         // Ausd
                         I64F64::from_num(0),                                         // Glmr
                         I64F64::from_num(0),                                         // Para
                         // Eth covariances
                         I64F64::from_num(15154913) / I64F64::from_num(1_000_000_000), // Eq
                         I64F64::from_num(0),                                          // Bnb
                         I64F64::from_num(13742562) / I64F64::from_num(1_000_000_000), // Btc
                         I64F64::from_num(0),                                          // Wbtc
                         I64F64::from_num(14555672) / I64F64::from_num(1_000_000_000), // Crv
                         I64F64::from_num(0),                                          // Dai
                         I64F64::from_num(15154913) / I64F64::from_num(1_000_000_000), // Dot
                         I64F64::from_num(10413835) / I64F64::from_num(1_000_000_000), // Eos
                         I64F64::from_num(17738244) / I64F64::from_num(1_000_000_000), // Eth
                         I64F64::from_num(0),                                          // Eqd
                         I64F64::from_num(0),                                          // Busd
                         I64F64::from_num(15154913) / I64F64::from_num(1_000_000_000), // Gens
                         I64F64::from_num(15154913) / I64F64::from_num(1_000_000_000), // Hdot
                         I64F64::from_num(0),                                          // Usdc
                         I64F64::from_num(0),                                          // Usdt
                         I64F64::from_num(15154913) / I64F64::from_num(1_000_000_000), // Xdot
                         I64F64::from_num(0),                                          // Aca
                         I64F64::from_num(0),                                          // Ausd
                         I64F64::from_num(0),                                          // Glmr
                         I64F64::from_num(0),                                          // Para
                         // Eqd covariances
                         I64F64::from_num(0), // Eq
                         I64F64::from_num(0), // Bnb
                         I64F64::from_num(0), // Btc
                         I64F64::from_num(0), // Wbtc
                         I64F64::from_num(0), // Crv
                         I64F64::from_num(0), // Dai
                         I64F64::from_num(0), // Dot
                         I64F64::from_num(0), // Eos
                         I64F64::from_num(0), // Eth
                         I64F64::from_num(0), // Eqd
                         I64F64::from_num(0), // Busd
                         I64F64::from_num(0), // Gens
                         I64F64::from_num(0), // Hdot
                         I64F64::from_num(0), // Usdc
                         I64F64::from_num(0), // Usdt
                         I64F64::from_num(0), // Xdot
                         I64F64::from_num(0), // Aca
                         I64F64::from_num(0), // Ausd
                         I64F64::from_num(0), // Glmr
                         I64F64::from_num(0), // Para
                         // Busd covariances
                         I64F64::from_num(0), // Eq
                         I64F64::from_num(0), // Bnb
                         I64F64::from_num(0), // Btc
                         I64F64::from_num(0), // Wbtc
                         I64F64::from_num(0), // Crv
                         I64F64::from_num(0), // Dai
                         I64F64::from_num(0), // Dot
                         I64F64::from_num(0), // Eos
                         I64F64::from_num(0), // Eth
                         I64F64::from_num(0), // Eqd
                         I64F64::from_num(0), // Busd
                         I64F64::from_num(0), // Gens
                         I64F64::from_num(0), // Hdot
                         I64F64::from_num(0), // Usdc
                         I64F64::from_num(0), // Usdt
                         I64F64::from_num(0), // Xdot
                         I64F64::from_num(0), // Aca
                         I64F64::from_num(0), // Ausd
                         I64F64::from_num(0), // Glmr
                         I64F64::from_num(0), // Para
                         // Gens asset covariances
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Eq
                         I64F64::from_num(0),                                          // Bnb
                         I64F64::from_num(11788778) / I64F64::from_num(1_000_000_000), // Btc
                         I64F64::from_num(0),                                          // Wbtc
                         I64F64::from_num(12345345) / I64F64::from_num(1_000_000_000), // Crv
                         I64F64::from_num(0),                                          // Dai
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Dot
                         I64F64::from_num(5824929) / I64F64::from_num(1_000_000_000),  // Eos
                         I64F64::from_num(15154913) / I64F64::from_num(1_000_000_000), // Eth
                         I64F64::from_num(0),                                          // Eqd
                         I64F64::from_num(0),                                          // Busd
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Gens
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Hdot
                         I64F64::from_num(0),                                          // Usdc
                         I64F64::from_num(0),                                          // Usdt
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Xdot
                         I64F64::from_num(0),                                          // Aca
                         I64F64::from_num(0),                                          // Ausd
                         I64F64::from_num(0),                                          // Glmr
                         I64F64::from_num(0),                                          // Para
                         // Hdot covariances
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Eq
                         I64F64::from_num(0),                                          // Bnb
                         I64F64::from_num(11788778) / I64F64::from_num(1_000_000_000), // Btc
                         I64F64::from_num(0),                                          // Wbtc
                         I64F64::from_num(13445676) / I64F64::from_num(1_000_000_000), // Crv
                         I64F64::from_num(0),                                          // Dai
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Dot
                         I64F64::from_num(5824929) / I64F64::from_num(1_000_000_000),  // Eos
                         I64F64::from_num(15154913) / I64F64::from_num(1_000_000_000), // Eth
                         I64F64::from_num(0),                                          // Eqd
                         I64F64::from_num(0),                                          // Busd
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Gens
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Hdot
                         I64F64::from_num(0),                                          // Usdc
                         I64F64::from_num(0),                                          // Usdt
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Xdot
                         I64F64::from_num(0),                                          // Aca
                         I64F64::from_num(0),                                          // Ausd
                         I64F64::from_num(0),                                          // Glmr
                         I64F64::from_num(0),                                          // Para
                         // Usdc covariances
                         I64F64::from_num(0), // Eq
                         I64F64::from_num(0), // Bnb
                         I64F64::from_num(0), // Btc
                         I64F64::from_num(0), // Wbtc
                         I64F64::from_num(0), // Crv
                         I64F64::from_num(0), // Dai
                         I64F64::from_num(0), // Dot
                         I64F64::from_num(0), // Eos
                         I64F64::from_num(0), // Eth
                         I64F64::from_num(0), // Eqd
                         I64F64::from_num(0), // Busd
                         I64F64::from_num(0), // Gens
                         I64F64::from_num(0), // Hdot
                         I64F64::from_num(0), // Usdc
                         I64F64::from_num(0), // Usdt
                         I64F64::from_num(0), // Xdot
                         I64F64::from_num(0), // Aca
                         I64F64::from_num(0), // Ausd
                         I64F64::from_num(0), // Glmr
                         I64F64::from_num(0), // Para
                         // Usdt covariances
                         I64F64::from_num(0), // Eq
                         I64F64::from_num(0), // Bnb
                         I64F64::from_num(0), // Btc
                         I64F64::from_num(0), // Wbtc
                         I64F64::from_num(0), // Crv
                         I64F64::from_num(0), // Dai
                         I64F64::from_num(0), // Dot
                         I64F64::from_num(0), // Eos
                         I64F64::from_num(0), // Eth
                         I64F64::from_num(0), // Eqd
                         I64F64::from_num(0), // Busd
                         I64F64::from_num(0), // Gens
                         I64F64::from_num(0), // Hdot
                         I64F64::from_num(0), // Usdc
                         I64F64::from_num(0), // Usdt
                         I64F64::from_num(0), // Xdot
                         I64F64::from_num(0), // Aca
                         I64F64::from_num(0), // Ausd
                         I64F64::from_num(0), // Glmr
                         I64F64::from_num(0), // Para
                         // Xdot covariances
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Eq
                         I64F64::from_num(0),                                          // Bnb
                         I64F64::from_num(11788778) / I64F64::from_num(1_000_000_000), // Btc
                         I64F64::from_num(0),                                          // Wbtc
                         I64F64::from_num(13445676) / I64F64::from_num(1_000_000_000), // Crv
                         I64F64::from_num(0),                                          // Dai
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Dot
                         I64F64::from_num(5824929) / I64F64::from_num(1_000_000_000),  // Eos
                         I64F64::from_num(15154913) / I64F64::from_num(1_000_000_000), // Eth
                         I64F64::from_num(0),                                          // Eqd
                         I64F64::from_num(0),                                          // Busd
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Gens
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Hdot
                         I64F64::from_num(0),                                          // Usdc
                         I64F64::from_num(0),                                          // Usdt
                         I64F64::from_num(17690330) / I64F64::from_num(1_000_000_000), // Xdot
                         I64F64::from_num(0),                                          // Aca
                         I64F64::from_num(0),                                          // Ausd
                         I64F64::from_num(0),                                          // Glmr
                         I64F64::from_num(0),                                          // Para
                         // Aca covariances
                         I64F64::from_num(0), // Eq
                         I64F64::from_num(0), // Bnb
                         I64F64::from_num(0), // Btc
                         I64F64::from_num(0), // Wbtc
                         I64F64::from_num(0), // Crv
                         I64F64::from_num(0), // Dai
                         I64F64::from_num(0), // Dot
                         I64F64::from_num(0), // Eos
                         I64F64::from_num(0), // Eth
                         I64F64::from_num(0), // Eqd
                         I64F64::from_num(0), // Busd
                         I64F64::from_num(0), // Gens
                         I64F64::from_num(0), // Hdot
                         I64F64::from_num(0), // Usdc
                         I64F64::from_num(0), // Usdt
                         I64F64::from_num(0), // Xdot
                         I64F64::from_num(0), // Aca
                         I64F64::from_num(0), // Ausd
                         I64F64::from_num(0), // Glmr
                         I64F64::from_num(0), // Para
                         // Ausd covariances
                         I64F64::from_num(0), // Eq
                         I64F64::from_num(0), // Bnb
                         I64F64::from_num(0), // Btc
                         I64F64::from_num(0), // Wbtc
                         I64F64::from_num(0), // Crv
                         I64F64::from_num(0), // Dai
                         I64F64::from_num(0), // Dot
                         I64F64::from_num(0), // Eos
                         I64F64::from_num(0), // Eth
                         I64F64::from_num(0), // Eqd
                         I64F64::from_num(0), // Busd
                         I64F64::from_num(0), // Gens
                         I64F64::from_num(0), // Hdot
                         I64F64::from_num(0), // Usdc
                         I64F64::from_num(0), // Usdt
                         I64F64::from_num(0), // Xdot
                         I64F64::from_num(0), // Aca
                         I64F64::from_num(0), // Ausd
                         I64F64::from_num(0), // Glmr
                         I64F64::from_num(0), // Para
                         // Glmr covariances
                         I64F64::from_num(0), // Eq
                         I64F64::from_num(0), // Bnb
                         I64F64::from_num(0), // Btc
                         I64F64::from_num(0), // Wbtc
                         I64F64::from_num(0), // Crv
                         I64F64::from_num(0), // Dai
                         I64F64::from_num(0), // Dot
                         I64F64::from_num(0), // Eos
                         I64F64::from_num(0), // Eth
                         I64F64::from_num(0), // Eqd
                         I64F64::from_num(0), // Busd
                         I64F64::from_num(0), // Gens
                         I64F64::from_num(0), // Hdot
                         I64F64::from_num(0), // Usdc
                         I64F64::from_num(0), // Usdt
                         I64F64::from_num(0), // Xdot
                         I64F64::from_num(0), // Aca
                         I64F64::from_num(0), // Ausd
                         I64F64::from_num(0), // Glmr
                         I64F64::from_num(0), // Para
                         // Para covariances
                         I64F64::from_num(0), // Eq
                         I64F64::from_num(0), // Bnb
                         I64F64::from_num(0), // Btc
                         I64F64::from_num(0), // Wbtc
                         I64F64::from_num(0), // Crv
                         I64F64::from_num(0), // Dai
                         I64F64::from_num(0), // Dot
                         I64F64::from_num(0), // Eos
                         I64F64::from_num(0), // Eth
                         I64F64::from_num(0), // Eqd
                         I64F64::from_num(0), // Busd
                         I64F64::from_num(0), // Gens
                         I64F64::from_num(0), // Hdot
                         I64F64::from_num(0), // Usdc
                         I64F64::from_num(0), // Usdt
                         I64F64::from_num(0), // Xdot
                         I64F64::from_num(0), // Aca
                         I64F64::from_num(0), // Ausd
                         I64F64::from_num(0), // Glmr
                         I64F64::from_num(0), // Para
                     ],*/
    }
}

pub fn get_per_asset_low_metrics(
    asset: Asset,
    period_start: Duration,
) -> AssetMetrics<Asset, I64F64> {
    let period_end = Duration::from_secs(period_start.as_secs() + 48 * 60);
    // ON_ADD_ASSET
    // correlations should be sorted by asset!
    // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
    match asset {
        asset::EQD => {
            let mut correlations = vec![
                (asset::EQ, I64F64::from_num(0)),
                (asset::BNB, I64F64::from_num(0)),
                (asset::BTC, I64F64::from_num(0)),
                (asset::WBTC, I64F64::from_num(0)),
                (asset::CRV, I64F64::from_num(0)),
                (asset::DAI, I64F64::from_num(0)),
                (asset::DOT, I64F64::from_num(0)),
                (asset::EOS, I64F64::from_num(0)),
                (asset::ETH, I64F64::from_num(0)),
                (asset::EQD, I64F64::from_num(1)),
                (asset::BUSD, I64F64::from_num(0)),
                (asset::GENS, I64F64::from_num(0)),
                (asset::HDOT, I64F64::from_num(0)),
                (asset::USDC, I64F64::from_num(0)),
                (asset::USDT, I64F64::from_num(0)),
                (asset::XDOT, I64F64::from_num(0)),
                (asset::ACA, I64F64::from_num(0)),
                (asset::AUSD, I64F64::from_num(0)),
                (asset::GLMR, I64F64::from_num(0)),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start: period_start.into(),
                period_end: period_end.into(),
                volatility: I64F64::from_num(0),
                returns: vec![],
                correlations,
            }
        }
        asset::EQ => {
            let mut correlations = vec![
                // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                (asset::EQ, I64F64::from_num(1)),
                (asset::BNB, I64F64::from_num(0)),
                (
                    asset::BTC,
                    I64F64::from_num(290984920) / I64F64::from_num(1_000_000_000),
                ),
                (asset::WBTC, I64F64::from_num(0)),
                (
                    asset::CRV,
                    I64F64::from_num(123005611) / I64F64::from_num(1_000_000_000),
                ),
                (asset::DAI, I64F64::from_num(0)),
                (asset::DOT, I64F64::from_num(1)),
                (asset::HDOT, I64F64::from_num(1)),
                (asset::XDOT, I64F64::from_num(1)),
                (
                    asset::EOS,
                    I64F64::from_num(67855854) / I64F64::from_num(1_000_000_000),
                ),
                (
                    asset::ETH,
                    I64F64::from_num(604728484) / I64F64::from_num(1_000_000_000),
                ),
                (asset::EQD, I64F64::from_num(0)),
                (asset::BUSD, I64F64::from_num(0)),
                (asset::GENS, I64F64::from_num(1)),
                (asset::USDC, I64F64::from_num(0)),
                (asset::USDT, I64F64::from_num(0)),
                (asset::ACA, I64F64::from_num(0)),
                (asset::AUSD, I64F64::from_num(0)),
                (asset::GLMR, I64F64::from_num(0)),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start: period_start.into(),
                period_end: period_end.into(),
                volatility: I64F64::from_num(1117135) / I64F64::from_num(1_000_000_000),
                returns: vec![],
                correlations,
            }
        }
        asset::ETH => {
            let mut correlations = vec![
                // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                (
                    asset::EQ,
                    I64F64::from_num(604728485) / I64F64::from_num(1_000_000_000),
                ),
                (asset::BNB, I64F64::from_num(0)),
                (
                    asset::BTC,
                    I64F64::from_num(106109657) / I64F64::from_num(1_000_000_000),
                ),
                (asset::WBTC, I64F64::from_num(0)),
                (
                    asset::CRV,
                    I64F64::from_num(345667454) / I64F64::from_num(1_000_000_000),
                ),
                (asset::DAI, I64F64::from_num(0)),
                (
                    asset::DOT,
                    I64F64::from_num(604728485) / I64F64::from_num(1_000_000_000),
                ),
                (
                    asset::HDOT,
                    I64F64::from_num(604728485) / I64F64::from_num(1_000_000_000),
                ),
                (
                    asset::XDOT,
                    I64F64::from_num(604728485) / I64F64::from_num(1_000_000_000),
                ),
                (
                    asset::EOS,
                    I64F64::from_num(167156595) / I64F64::from_num(1_000_000_000),
                ),
                (asset::ETH, I64F64::from_num(1)),
                (asset::EQD, I64F64::from_num(0)),
                (asset::BUSD, I64F64::from_num(0)),
                (
                    asset::GENS,
                    I64F64::from_num(604728485) / I64F64::from_num(1_000_000_000),
                ),
                (asset::USDC, I64F64::from_num(0)),
                (asset::USDT, I64F64::from_num(0)),
                (asset::ACA, I64F64::from_num(0)),
                (asset::AUSD, I64F64::from_num(0)),
                (asset::GLMR, I64F64::from_num(0)),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start: period_start.into(),
                period_end: period_end.into(),
                volatility: I64F64::from_num(17990) / I64F64::from_num(1_000_000),
                returns: vec![],
                correlations,
            }
        }
        asset::BTC => {
            let mut correlations = vec![
                // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                (
                    asset::EQ,
                    I64F64::from_num(290984920) / I64F64::from_num(1_000_000_000),
                ),
                (asset::BNB, I64F64::from_num(0)),
                (asset::BTC, I64F64::from_num(1)),
                (asset::WBTC, I64F64::from_num(0)),
                (
                    asset::CRV,
                    I64F64::from_num(567368074) / I64F64::from_num(1_000_000_000),
                ),
                (asset::DAI, I64F64::from_num(0)),
                (
                    asset::DOT,
                    I64F64::from_num(290984920) / I64F64::from_num(1_000_000_000),
                ),
                (
                    asset::HDOT,
                    I64F64::from_num(290984920) / I64F64::from_num(1_000_000_000),
                ),
                (
                    asset::XDOT,
                    I64F64::from_num(290984920) / I64F64::from_num(1_000_000_000),
                ),
                (
                    asset::EOS,
                    I64F64::from_num(76345121) / I64F64::from_num(1_000_000_000),
                ),
                (
                    asset::ETH,
                    I64F64::from_num(106109657) / I64F64::from_num(1_000_000_000),
                ),
                (asset::EQD, I64F64::from_num(0)),
                (asset::BUSD, I64F64::from_num(0)),
                (
                    asset::GENS,
                    I64F64::from_num(290984920) / I64F64::from_num(1_000_000_000),
                ),
                (asset::USDC, I64F64::from_num(0)),
                (asset::USDT, I64F64::from_num(0)),
                (asset::ACA, I64F64::from_num(0)),
                (asset::AUSD, I64F64::from_num(0)),
                (asset::GLMR, I64F64::from_num(0)),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start: period_start.into(),
                period_end: period_end.into(),
                volatility: I64F64::from_num(263) / I64F64::from_num(1_000_000_000),
                returns: vec![],
                correlations,
            }
        }
        asset::EOS => {
            let mut correlations = vec![
                // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                (
                    asset::EQ,
                    I64F64::from_num(67855855) / I64F64::from_num(1_000_000_000),
                ),
                (asset::BNB, I64F64::from_num(0)),
                (
                    asset::BTC,
                    I64F64::from_num(76345121) / I64F64::from_num(1_000_000_000),
                ),
                (asset::WBTC, I64F64::from_num(0)),
                (
                    asset::CRV,
                    I64F64::from_num(357943565) / I64F64::from_num(1_000_000_000),
                ),
                (asset::DAI, I64F64::from_num(0)),
                (
                    asset::DOT,
                    I64F64::from_num(67855855) / I64F64::from_num(1_000_000_000),
                ),
                (
                    asset::HDOT,
                    I64F64::from_num(67855855) / I64F64::from_num(1_000_000_000),
                ),
                (
                    asset::XDOT,
                    I64F64::from_num(67855855) / I64F64::from_num(1_000_000_000),
                ),
                (asset::EOS, I64F64::from_num(1)),
                (
                    asset::ETH,
                    I64F64::from_num(167156595) / I64F64::from_num(1_000_000_000),
                ),
                (asset::EQD, I64F64::from_num(0)),
                (asset::BUSD, I64F64::from_num(0)),
                (
                    asset::GENS,
                    I64F64::from_num(67855855) / I64F64::from_num(1_000_000_000),
                ),
                (asset::USDC, I64F64::from_num(0)),
                (asset::USDT, I64F64::from_num(0)),
                (asset::ACA, I64F64::from_num(0)),
                (asset::AUSD, I64F64::from_num(0)),
                (asset::GLMR, I64F64::from_num(0)),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start: period_start.into(),
                period_end: period_end.into(),
                volatility: I64F64::from_num(15307805) / I64F64::from_num(1_000_000_000),
                returns: vec![],
                correlations,
            }
        }
        asset::DOT | asset::HDOT | asset::XDOT => {
            let mut correlations = vec![
                // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                (asset::EQ, I64F64::from_num(1)),
                (asset::BNB, I64F64::from_num(0)),
                (
                    asset::BTC,
                    I64F64::from_num(290984920) / I64F64::from_num(1_000_000_000),
                ),
                (asset::WBTC, I64F64::from_num(0)),
                (
                    asset::CRV,
                    I64F64::from_num(679456356) / I64F64::from_num(1_000_000_000),
                ),
                (asset::DAI, I64F64::from_num(0)),
                (asset::DOT, I64F64::from_num(1)),
                (asset::HDOT, I64F64::from_num(1)),
                (asset::XDOT, I64F64::from_num(1)),
                (
                    asset::EOS,
                    I64F64::from_num(67855855) / I64F64::from_num(1_000_000_000),
                ),
                (
                    asset::ETH,
                    I64F64::from_num(604728485) / I64F64::from_num(1_000_000_000),
                ),
                (asset::EQD, I64F64::from_num(0)),
                (asset::BUSD, I64F64::from_num(0)),
                (asset::GENS, I64F64::from_num(1)),
                (asset::USDC, I64F64::from_num(0)),
                (asset::USDT, I64F64::from_num(0)),
                (asset::ACA, I64F64::from_num(0)),
                (asset::AUSD, I64F64::from_num(0)),
                (asset::GLMR, I64F64::from_num(0)),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start: period_start.into(),
                period_end: period_end.into(),
                volatility: I64F64::from_num(1117135) / I64F64::from_num(1_000_000_000),
                returns: vec![],
                correlations,
            }
        }
        asset::CRV => {
            let mut correlations = vec![
                // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                (
                    asset::EQ,
                    I64F64::from_num(123005611) / I64F64::from_num(1_000_000_000),
                ),
                (asset::BNB, I64F64::from_num(0)),
                (
                    asset::BTC,
                    I64F64::from_num(567368074) / I64F64::from_num(1_000_000_000),
                ),
                (asset::WBTC, I64F64::from_num(0)),
                (asset::CRV, I64F64::from_num(1)),
                (asset::DAI, I64F64::from_num(0)),
                (
                    asset::DOT,
                    I64F64::from_num(679456356) / I64F64::from_num(1_000_000_000),
                ),
                (
                    asset::HDOT,
                    I64F64::from_num(679456356) / I64F64::from_num(1_000_000_000),
                ),
                (
                    asset::XDOT,
                    I64F64::from_num(679456356) / I64F64::from_num(1_000_000_000),
                ),
                (
                    asset::EOS,
                    I64F64::from_num(357943565) / I64F64::from_num(1_000_000_000),
                ),
                (
                    asset::ETH,
                    I64F64::from_num(345667454) / I64F64::from_num(1_000_000_000),
                ),
                (asset::EQD, I64F64::from_num(0)),
                (asset::BUSD, I64F64::from_num(0)),
                (
                    asset::GENS,
                    I64F64::from_num(123005611) / I64F64::from_num(1_000_000_000),
                ),
                (asset::USDC, I64F64::from_num(0)),
                (asset::USDT, I64F64::from_num(0)),
                (asset::ACA, I64F64::from_num(0)),
                (asset::AUSD, I64F64::from_num(0)),
                (asset::GLMR, I64F64::from_num(0)),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start: period_start.into(),
                period_end: period_end.into(),
                volatility: I64F64::from_num(333) / I64F64::from_num(1_000_000_000),
                returns: vec![],
                correlations,
            }
        }
        asset::GENS => {
            let mut correlations = vec![
                // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                (asset::EQ, I64F64::from_num(1)),
                (asset::BNB, I64F64::from_num(0)),
                (
                    asset::BTC,
                    I64F64::from_num(290984920) / I64F64::from_num(1_000_000_000),
                ),
                (asset::WBTC, I64F64::from_num(0)),
                (
                    asset::CRV,
                    I64F64::from_num(123005611) / I64F64::from_num(1_000_000_000),
                ),
                (asset::DAI, I64F64::from_num(0)),
                (asset::DOT, I64F64::from_num(1)),
                (asset::HDOT, I64F64::from_num(1)),
                (asset::XDOT, I64F64::from_num(1)),
                (
                    asset::EOS,
                    I64F64::from_num(67855854) / I64F64::from_num(1_000_000_000),
                ),
                (
                    asset::ETH,
                    I64F64::from_num(604728484) / I64F64::from_num(1_000_000_000),
                ),
                (asset::EQD, I64F64::from_num(0)),
                (asset::BUSD, I64F64::from_num(0)),
                (asset::GENS, I64F64::from_num(1)),
                (asset::USDC, I64F64::from_num(0)),
                (asset::USDT, I64F64::from_num(0)),
                (asset::ACA, I64F64::from_num(0)),
                (asset::AUSD, I64F64::from_num(0)),
                (asset::GLMR, I64F64::from_num(0)),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start: period_start.into(),
                period_end: period_end.into(),
                volatility: I64F64::from_num(1117135) / I64F64::from_num(1_000_000_000),
                returns: vec![],
                correlations,
            }
        }
        asset::DAI => {
            let mut correlations = vec![
                // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                (asset::EQ, I64F64::from_num(0)),
                (asset::BNB, I64F64::from_num(0)),
                (asset::BTC, I64F64::from_num(0)),
                (asset::WBTC, I64F64::from_num(0)),
                (asset::CRV, I64F64::from_num(0)),
                (asset::DAI, I64F64::from_num(1)),
                (asset::DOT, I64F64::from_num(0)),
                (asset::HDOT, I64F64::from_num(0)),
                (asset::XDOT, I64F64::from_num(0)),
                (asset::EOS, I64F64::from_num(0)),
                (asset::ETH, I64F64::from_num(0)),
                (asset::EQD, I64F64::from_num(0)),
                (asset::BUSD, I64F64::from_num(0)),
                (asset::GENS, I64F64::from_num(0)),
                (asset::USDC, I64F64::from_num(0)),
                (asset::USDT, I64F64::from_num(0)),
                (asset::ACA, I64F64::from_num(0)),
                (asset::AUSD, I64F64::from_num(0)),
                (asset::GLMR, I64F64::from_num(0)),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start: period_start.into(),
                period_end: period_end.into(),
                volatility: I64F64::from_num(0),
                returns: vec![],
                correlations,
            }
        }
        asset::BUSD => {
            let mut correlations = vec![
                // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                (asset::EQ, I64F64::from_num(0)),
                (asset::BNB, I64F64::from_num(0)),
                (asset::BTC, I64F64::from_num(0)),
                (asset::WBTC, I64F64::from_num(0)),
                (asset::CRV, I64F64::from_num(0)),
                (asset::DAI, I64F64::from_num(0)),
                (asset::DOT, I64F64::from_num(0)),
                (asset::HDOT, I64F64::from_num(0)),
                (asset::XDOT, I64F64::from_num(0)),
                (asset::EOS, I64F64::from_num(0)),
                (asset::ETH, I64F64::from_num(0)),
                (asset::EQD, I64F64::from_num(0)),
                (asset::BUSD, I64F64::from_num(1)),
                (asset::GENS, I64F64::from_num(0)),
                (asset::USDC, I64F64::from_num(0)),
                (asset::USDT, I64F64::from_num(0)),
                (asset::ACA, I64F64::from_num(0)),
                (asset::AUSD, I64F64::from_num(0)),
                (asset::GLMR, I64F64::from_num(0)),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start: period_start.into(),
                period_end: period_end.into(),
                volatility: I64F64::from_num(0),
                returns: vec![],
                correlations,
            }
        }
        asset::USDC => {
            let mut correlations = vec![
                // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                (asset::EQ, I64F64::from_num(0)),
                (asset::BNB, I64F64::from_num(0)),
                (asset::BTC, I64F64::from_num(0)),
                (asset::WBTC, I64F64::from_num(0)),
                (asset::CRV, I64F64::from_num(0)),
                (asset::DAI, I64F64::from_num(0)),
                (asset::DOT, I64F64::from_num(0)),
                (asset::HDOT, I64F64::from_num(0)),
                (asset::XDOT, I64F64::from_num(0)),
                (asset::EOS, I64F64::from_num(0)),
                (asset::ETH, I64F64::from_num(0)),
                (asset::EQD, I64F64::from_num(0)),
                (asset::BUSD, I64F64::from_num(0)),
                (asset::GENS, I64F64::from_num(0)),
                (asset::USDC, I64F64::from_num(1)),
                (asset::USDT, I64F64::from_num(0)),
                (asset::ACA, I64F64::from_num(0)),
                (asset::AUSD, I64F64::from_num(0)),
                (asset::GLMR, I64F64::from_num(0)),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start: period_start.into(),
                period_end: period_end.into(),
                volatility: I64F64::from_num(0),
                returns: vec![],
                correlations,
            }
        }
        asset::USDT => {
            let mut correlations = vec![
                // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                (asset::EQ, I64F64::from_num(0)),
                (asset::BNB, I64F64::from_num(0)),
                (asset::BTC, I64F64::from_num(0)),
                (asset::WBTC, I64F64::from_num(0)),
                (asset::CRV, I64F64::from_num(0)),
                (asset::DAI, I64F64::from_num(0)),
                (asset::DOT, I64F64::from_num(0)),
                (asset::HDOT, I64F64::from_num(0)),
                (asset::XDOT, I64F64::from_num(0)),
                (asset::EOS, I64F64::from_num(0)),
                (asset::ETH, I64F64::from_num(0)),
                (asset::EQD, I64F64::from_num(0)),
                (asset::BUSD, I64F64::from_num(0)),
                (asset::GENS, I64F64::from_num(0)),
                (asset::USDC, I64F64::from_num(0)),
                (asset::USDT, I64F64::from_num(1)),
                (asset::ACA, I64F64::from_num(0)),
                (asset::AUSD, I64F64::from_num(0)),
                (asset::GLMR, I64F64::from_num(0)),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start: period_start.into(),
                period_end: period_end.into(),
                volatility: I64F64::from_num(0),
                returns: vec![],
                correlations,
            }
        }
        asset::BNB => {
            let mut correlations = vec![
                // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                (asset::EQ, I64F64::from_num(0)),
                (asset::BNB, I64F64::from_num(1)),
                (asset::BTC, I64F64::from_num(0)),
                (asset::WBTC, I64F64::from_num(0)),
                (asset::CRV, I64F64::from_num(0)),
                (asset::DAI, I64F64::from_num(0)),
                (asset::DOT, I64F64::from_num(0)),
                (asset::HDOT, I64F64::from_num(0)),
                (asset::XDOT, I64F64::from_num(0)),
                (asset::EOS, I64F64::from_num(0)),
                (asset::ETH, I64F64::from_num(0)),
                (asset::EQD, I64F64::from_num(0)),
                (asset::BUSD, I64F64::from_num(0)),
                (asset::GENS, I64F64::from_num(0)),
                (asset::USDC, I64F64::from_num(0)),
                (asset::USDT, I64F64::from_num(0)),
                (asset::ACA, I64F64::from_num(0)),
                (asset::AUSD, I64F64::from_num(0)),
                (asset::GLMR, I64F64::from_num(0)),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start: period_start.into(),
                period_end: period_end.into(),
                volatility: I64F64::from_num(0),
                returns: vec![],
                correlations,
            }
        }
        asset::WBTC => {
            let mut correlations = vec![
                // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                (asset::EQ, I64F64::from_num(0)),
                (asset::BNB, I64F64::from_num(0)),
                (asset::BTC, I64F64::from_num(0)),
                (asset::WBTC, I64F64::from_num(1)),
                (asset::CRV, I64F64::from_num(0)),
                (asset::DAI, I64F64::from_num(0)),
                (asset::DOT, I64F64::from_num(0)),
                (asset::HDOT, I64F64::from_num(0)),
                (asset::XDOT, I64F64::from_num(0)),
                (asset::EOS, I64F64::from_num(0)),
                (asset::ETH, I64F64::from_num(0)),
                (asset::EQD, I64F64::from_num(0)),
                (asset::BUSD, I64F64::from_num(0)),
                (asset::GENS, I64F64::from_num(0)),
                (asset::USDC, I64F64::from_num(0)),
                (asset::USDT, I64F64::from_num(0)),
                (asset::ACA, I64F64::from_num(0)),
                (asset::AUSD, I64F64::from_num(0)),
                (asset::GLMR, I64F64::from_num(0)),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start: period_start.into(),
                period_end: period_end.into(),
                volatility: I64F64::from_num(0),
                returns: vec![],
                correlations,
            }
        }
        asset::ACA => {
            let mut correlations = vec![
                // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                (asset::EQ, I64F64::from_num(0)),
                (asset::BNB, I64F64::from_num(0)),
                (asset::BTC, I64F64::from_num(0)),
                (asset::WBTC, I64F64::from_num(0)),
                (asset::CRV, I64F64::from_num(0)),
                (asset::DAI, I64F64::from_num(0)),
                (asset::DOT, I64F64::from_num(0)),
                (asset::HDOT, I64F64::from_num(0)),
                (asset::XDOT, I64F64::from_num(0)),
                (asset::EOS, I64F64::from_num(0)),
                (asset::ETH, I64F64::from_num(0)),
                (asset::EQD, I64F64::from_num(0)),
                (asset::BUSD, I64F64::from_num(0)),
                (asset::GENS, I64F64::from_num(0)),
                (asset::USDC, I64F64::from_num(0)),
                (asset::USDT, I64F64::from_num(0)),
                (asset::ACA, I64F64::from_num(1)),
                (asset::AUSD, I64F64::from_num(0)),
                (asset::GLMR, I64F64::from_num(0)),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start: period_start.into(),
                period_end: period_end.into(),
                volatility: I64F64::from_num(0),
                returns: vec![],
                correlations,
            }
        }
        asset::AUSD => {
            let mut correlations = vec![
                // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                (asset::EQ, I64F64::from_num(0)),
                (asset::BNB, I64F64::from_num(0)),
                (asset::BTC, I64F64::from_num(0)),
                (asset::WBTC, I64F64::from_num(0)),
                (asset::CRV, I64F64::from_num(0)),
                (asset::DAI, I64F64::from_num(0)),
                (asset::DOT, I64F64::from_num(0)),
                (asset::HDOT, I64F64::from_num(0)),
                (asset::XDOT, I64F64::from_num(0)),
                (asset::EOS, I64F64::from_num(0)),
                (asset::ETH, I64F64::from_num(0)),
                (asset::EQD, I64F64::from_num(0)),
                (asset::BUSD, I64F64::from_num(0)),
                (asset::GENS, I64F64::from_num(0)),
                (asset::USDC, I64F64::from_num(0)),
                (asset::USDT, I64F64::from_num(0)),
                (asset::ACA, I64F64::from_num(0)),
                (asset::AUSD, I64F64::from_num(1)),
                (asset::GLMR, I64F64::from_num(0)),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start: period_start.into(),
                period_end: period_end.into(),
                volatility: I64F64::from_num(0),
                returns: vec![],
                correlations,
            }
        }
        asset::GLMR => {
            let mut correlations = vec![
                // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                (asset::EQ, I64F64::from_num(0)),
                (asset::BNB, I64F64::from_num(0)),
                (asset::BTC, I64F64::from_num(0)),
                (asset::WBTC, I64F64::from_num(0)),
                (asset::CRV, I64F64::from_num(0)),
                (asset::DAI, I64F64::from_num(0)),
                (asset::DOT, I64F64::from_num(0)),
                (asset::HDOT, I64F64::from_num(0)),
                (asset::XDOT, I64F64::from_num(0)),
                (asset::EOS, I64F64::from_num(0)),
                (asset::ETH, I64F64::from_num(0)),
                (asset::EQD, I64F64::from_num(0)),
                (asset::BUSD, I64F64::from_num(0)),
                (asset::GENS, I64F64::from_num(0)),
                (asset::USDC, I64F64::from_num(0)),
                (asset::USDT, I64F64::from_num(0)),
                (asset::ACA, I64F64::from_num(0)),
                (asset::AUSD, I64F64::from_num(0)),
                (asset::GLMR, I64F64::from_num(1)),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start: period_start.into(),
                period_end: period_end.into(),
                volatility: I64F64::from_num(0),
                returns: vec![],
                correlations,
            }
        }
        asset::PARA => {
            let mut correlations = vec![
                // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                (asset::EQ, I64F64::from_num(0)),
                (asset::BNB, I64F64::from_num(0)),
                (asset::BTC, I64F64::from_num(0)),
                (asset::WBTC, I64F64::from_num(0)),
                (asset::CRV, I64F64::from_num(0)),
                (asset::DAI, I64F64::from_num(0)),
                (asset::DOT, I64F64::from_num(0)),
                (asset::HDOT, I64F64::from_num(0)),
                (asset::XDOT, I64F64::from_num(0)),
                (asset::EOS, I64F64::from_num(0)),
                (asset::ETH, I64F64::from_num(0)),
                (asset::EQD, I64F64::from_num(0)),
                (asset::BUSD, I64F64::from_num(0)),
                (asset::GENS, I64F64::from_num(0)),
                (asset::USDC, I64F64::from_num(0)),
                (asset::USDT, I64F64::from_num(0)),
                (asset::ACA, I64F64::from_num(0)),
                (asset::AUSD, I64F64::from_num(0)),
                (asset::GLMR, I64F64::from_num(0)),
                (asset::PARA, I64F64::from_num(1)),
                (asset::EQDOT, I64F64::from_num(0)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start: period_start.into(),
                period_end: period_end.into(),
                volatility: I64F64::from_num(0),
                returns: vec![],
                correlations,
            }
        }
        asset::EQDOT => {
            let mut correlations = vec![
                // $EQ, $BNB, $BTC, $WBTC, $CRV, $DAI, $DOT, $EOS, $ETH, $EQD, $BUSD, $GENS, $HDOT, $USDC, $USDT, $XDOT
                (asset::EQ, I64F64::from_num(0)),
                (asset::BNB, I64F64::from_num(0)),
                (asset::BTC, I64F64::from_num(0)),
                (asset::WBTC, I64F64::from_num(0)),
                (asset::CRV, I64F64::from_num(0)),
                (asset::DAI, I64F64::from_num(0)),
                (asset::DOT, I64F64::from_num(0)),
                (asset::HDOT, I64F64::from_num(0)),
                (asset::XDOT, I64F64::from_num(0)),
                (asset::EOS, I64F64::from_num(0)),
                (asset::ETH, I64F64::from_num(0)),
                (asset::EQD, I64F64::from_num(0)),
                (asset::BUSD, I64F64::from_num(0)),
                (asset::GENS, I64F64::from_num(0)),
                (asset::USDC, I64F64::from_num(0)),
                (asset::USDT, I64F64::from_num(0)),
                (asset::ACA, I64F64::from_num(0)),
                (asset::AUSD, I64F64::from_num(0)),
                (asset::GLMR, I64F64::from_num(0)),
                (asset::PARA, I64F64::from_num(0)),
                (asset::EQDOT, I64F64::from_num(1)),
            ];
            correlations.sort_by(|(a1, _), (a2, _)| a1.cmp(a2));
            AssetMetrics {
                period_start: period_start.into(),
                period_end: period_end.into(),
                volatility: I64F64::from_num(0),
                returns: vec![],
                correlations,
            }
        }
        a => {
            panic!("Exhausted options. Asset: {:?}", a)
        }
    }
}

pub fn get_per_asset_discount_metrics(
    asset: Asset,
    period_start: Duration,
) -> AssetMetrics<Asset, I64F64> {
    let period_end = Duration::from_secs(period_start.as_secs() + 48 * 60);

    let volatility = match asset {
        asset::DOT => I64F64::from_str("0.0800").unwrap(),
        asset::GENS => I64F64::from_str("0.1000").unwrap(),
        asset::USDT => I64F64::from_str("0.0001").unwrap(),
        asset::ETH => I64F64::from_str("0.0600").unwrap(),
        asset::EQD => I64F64::from_str("0.0002").unwrap(),
        _ => I64F64::from_num(0),
    };

    fn correlation(a0: Asset, a1: Asset) -> I64F64 {
        let (max, min) = match a0.cmp(&a1) {
            core::cmp::Ordering::Less => (a1, a0),
            core::cmp::Ordering::Equal => return I64F64::from_num(1),
            core::cmp::Ordering::Greater => (a0, a1),
        };

        // USDT > GENS > EQD > ETH > DOT

        match (max, min) {
            (asset::USDT, asset::EQD) => I64F64::from_str("1").unwrap(),
            (asset::USDT, asset::ETH) => I64F64::from_str("-0.1").unwrap(),
            (asset::GENS, asset::DOT) => I64F64::from_str("0.8").unwrap(),
            (asset::GENS, asset::ETH) => I64F64::from_str("0.5").unwrap(),
            (asset::ETH, asset::DOT) => I64F64::from_str("0.6").unwrap(),
            _ => I64F64::from_num(0),
        }
    }

    let assets = vec![
        asset::DOT,
        asset::GENS,
        asset::USDT,
        asset::ETH,
        asset::EQD,
        asset::EQ,
        asset::BNB,
        asset::BTC,
        asset::WBTC,
        asset::CRV,
        asset::DAI,
        asset::HDOT,
        asset::XDOT,
        asset::EOS,
        asset::BUSD,
        asset::USDC,
        asset::ACA,
        asset::AUSD,
        asset::GLMR,
        asset::PARA,
        asset::EQDOT,
    ];

    AssetMetrics {
        period_start: period_start.into(),
        period_end: period_end.into(),
        volatility,
        returns: vec![],
        correlations: assets
            .into_iter()
            .map(|a| (a, correlation(a, asset)))
            .collect(),
    }
}

pub fn get_discount_metrics(period_start: Duration) -> FinancialMetrics<Asset, I64F64> {
    let period_end = Duration::from_secs(period_start.as_secs() + 48 * 60);
    let assets = vec![
        asset::DOT,
        asset::GENS,
        asset::USDT,
        asset::ETH,
        asset::EQD,
        asset::EQ,
        asset::BNB,
        asset::BTC,
        asset::WBTC,
        asset::CRV,
        asset::DAI,
        asset::HDOT,
        asset::XDOT,
        asset::EOS,
        asset::BUSD,
        asset::USDC,
        asset::ACA,
        asset::AUSD,
        asset::GLMR,
        asset::PARA,
        asset::EQDOT,
    ];

    let volatilities = vec![
        I64F64::from_str("0.0800").unwrap(),
        I64F64::from_str("0.1000").unwrap(),
        I64F64::from_str("0.0001").unwrap(),
        I64F64::from_str("0.0600").unwrap(),
        I64F64::from_str("0.0002").unwrap(),
        I64F64::from_num(0),
        I64F64::from_num(0),
        I64F64::from_num(0),
        I64F64::from_num(0),
        I64F64::from_num(0),
        I64F64::from_num(0),
        I64F64::from_num(0),
        I64F64::from_num(0),
        I64F64::from_num(0),
        I64F64::from_num(0),
        I64F64::from_num(0),
        I64F64::from_num(0),
        I64F64::from_num(0),
        I64F64::from_num(0),
        I64F64::from_num(0),
        I64F64::from_num(0),
    ];

    fn correlation(a0: Asset, a1: Asset) -> I64F64 {
        let (max, min) = match a0.cmp(&a1) {
            core::cmp::Ordering::Less => (a1, a0),
            core::cmp::Ordering::Equal => return I64F64::from_num(1),
            core::cmp::Ordering::Greater => (a0, a1),
        };

        // USDT > GENS > EQD > ETH > DOT

        match (max, min) {
            (asset::USDT, asset::EQD) => I64F64::from_str("1").unwrap(),
            (asset::USDT, asset::ETH) => I64F64::from_str("-0.1").unwrap(),
            (asset::GENS, asset::DOT) => I64F64::from_str("0.8").unwrap(),
            (asset::GENS, asset::ETH) => I64F64::from_str("0.5").unwrap(),
            (asset::ETH, asset::DOT) => I64F64::from_str("0.6").unwrap(),
            _ => I64F64::from_num(0),
        }
    }
    let correlations: Vec<I64F64> = assets
        .iter()
        .flat_map(|a0| assets.iter().map(move |a1| correlation(*a0, *a1)))
        .collect();

    let covariances = (0..assets.len())
        .into_iter()
        .flat_map(|idx| {
            (0..assets.len())
                .into_iter()
                .map(|idy| {
                    correlations[idx * assets.len() + idy] * volatilities[idx] * volatilities[idy]
                })
                .collect::<Vec<_>>()
        })
        .collect();

    FinancialMetrics {
        period_start: period_start.into(),
        period_end: period_end.into(),
        assets,
        mean_returns: vec![],
        volatilities,
        correlations,
        covariances,
    }
}

pub fn get_low_metrics(period_start: Duration) -> FinancialMetrics<Asset, I64F64> {
    let period_end = Duration::from_secs(period_start.as_secs() + 48 * 60);
    let mut assets = vec![
        asset::EQ,
        asset::BNB,
        asset::BTC,
        asset::WBTC,
        asset::CRV,
        asset::DAI,
        asset::DOT,
        asset::HDOT,
        asset::XDOT,
        asset::EOS,
        asset::ETH,
        asset::EQD,
        asset::BUSD,
        asset::GENS,
        asset::USDC,
        asset::USDT,
        asset::ACA,
        asset::AUSD,
        asset::GLMR,
        asset::PARA,
        asset::EQDOT,
    ];
    assets.sort_by(|a1, a2| a1.cmp(a2));
    let volatilities: Vec<_> = assets
        .iter()
        .map(|a| get_per_asset_low_metrics(*a, period_start).volatility)
        .collect();
    let correlations: Vec<_> = assets
        .iter()
        .map(|a| {
            get_per_asset_low_metrics(*a, period_start)
                .correlations
                .iter()
                .map(|(_, c)| *c)
                .collect::<Vec<I64F64>>()
        })
        .flatten()
        .collect();

    let covariances: Vec<_> = (0..assets.len())
        .into_iter()
        .flat_map(|idx| {
            (0..assets.len())
                .into_iter()
                .map(|idy| {
                    correlations[idx * assets.len() + idy] * volatilities[idx] * volatilities[idy]
                })
                .collect::<Vec<_>>()
        })
        .collect();

    FinancialMetrics {
        period_start: period_start.into(),
        period_end: period_end.into(),
        assets,
        mean_returns: vec![],
        volatilities,
        correlations,
        covariances,
    }
}
