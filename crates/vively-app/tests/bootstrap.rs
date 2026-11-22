use std::sync::Arc;

use vively_app::{
    CoreConfig, CorePaths, FileSessionPersistence, Host, TokioBridge, build_core,
};
use vively_core::downloads::{NetworkInterface, NetworkState, NetworkStatus};

struct NopHost;

impl Host for NopHost {
    fn wake(&self) {}
    fn network_state(&self) -> NetworkState {
        NetworkState {
            status: NetworkStatus::Online(NetworkInterface::Wifi),
            expensive: false,
            constrained: false,
        }
    }
}

#[test]
fn build_core_from_empty_dir_creates_device_and_starts_signed_out() {
    let root = std::env::temp_dir().join(format!("vively-bootstrap-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let paths = CorePaths::from_root(&root);
    let core = build_core(
        Arc::new(NopHost),
        TokioBridge::new().unwrap(),
        CoreConfig {
            persistence: Arc::new(FileSessionPersistence {
                path: paths.session_file(),
            }),
            paths: paths.clone(),
            simulated_audio: true,
            qr_device_name: "Vively (test)".into(),
            client_secret: "test-secret".into(),
            dev_session: None,
            api_host: None,
        },
    )
    .unwrap();
    assert!(core.session.is_none());
    assert!(paths.device_file().exists());
    let _ = std::fs::remove_dir_all(&root);
}
