//! Bonjour/mDNS discovery with memory-only endpoints.
//!
//! The advertised instance and host names are random. Device names, identity
//! fingerprints, addresses and ports are never written to SQLite or transfer
//! summaries.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};

use super::PROTOCOL_VERSION;

pub(crate) const SERVICE_TYPE: &str = "_clipriva._tcp.local.";

/// An opaque, process-only handle for one Bonjour service. It deliberately
/// does not carry a hostname, IP address, port, device name, or identity.
pub(crate) struct DiscoveryCandidate {
    pub(crate) id: String,
    pub(crate) endpoints: Vec<SocketAddr>,
}

#[derive(Clone)]
struct DiscoveredService {
    endpoints: Vec<SocketAddr>,
    pairing_visible: bool,
}

pub(crate) struct DiscoveryRuntime {
    daemon: ServiceDaemon,
    instance_name: String,
    host_name: String,
    full_name: String,
    port: u16,
    endpoints: Arc<Mutex<HashMap<String, DiscoveredService>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl DiscoveryRuntime {
    pub(crate) fn start(port: u16) -> AppResult<Self> {
        let daemon = ServiceDaemon::new().map_err(discovery_error)?;
        let token = uuid::Uuid::new_v4().simple().to_string();
        let instance_name = format!("ll-{token}");
        let host_name = format!("ll-{token}.local.");
        let service = service_info(&instance_name, &host_name, port, false)?;
        let full_name = service.get_fullname().to_owned();
        daemon.register(service).map_err(discovery_error)?;
        let receiver = daemon.browse(SERVICE_TYPE).map_err(discovery_error)?;
        let endpoints = Arc::new(Mutex::new(HashMap::new()));
        let worker_endpoints = Arc::clone(&endpoints);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let own_full_name = full_name.clone();
        let worker = thread::Builder::new()
            .name("clipriva-local-link-mdns".to_owned())
            .spawn(move || {
                while !worker_stop.load(Ordering::Acquire) {
                    match receiver.recv_timeout(Duration::from_millis(250)) {
                        Ok(ServiceEvent::ServiceResolved(service))
                            if service.get_fullname() != own_full_name
                                && service
                                    .get_property_val_str("v")
                                    .and_then(|value| value.parse::<u16>().ok())
                                    == Some(PROTOCOL_VERSION) =>
                        {
                            let resolved = service
                                .get_addresses_v4()
                                .into_iter()
                                .map(|address| {
                                    SocketAddr::new(IpAddr::V4(address), service.get_port())
                                })
                                .collect::<Vec<_>>();
                            if !resolved.is_empty() {
                                worker_endpoints
                                    .lock()
                                    .expect("Local Link endpoint mutex poisoned")
                                    .insert(
                                        service.get_fullname().to_owned(),
                                        DiscoveredService {
                                            endpoints: resolved,
                                            pairing_visible: service.get_property_val_str("pair")
                                                == Some("1"),
                                        },
                                    );
                            }
                        }
                        Ok(ServiceEvent::ServiceRemoved(_, full_name)) => {
                            worker_endpoints
                                .lock()
                                .expect("Local Link endpoint mutex poisoned")
                                .remove(&full_name);
                        }
                        Ok(_) | Err(_) => {}
                    }
                }
            })?;
        Ok(Self {
            daemon,
            instance_name,
            host_name,
            full_name,
            port,
            endpoints,
            stop,
            worker: Some(worker),
        })
    }

    pub(crate) fn candidates(&self) -> Vec<DiscoveryCandidate> {
        let mut candidates = self
            .endpoints
            .lock()
            .expect("Local Link endpoint mutex poisoned")
            .iter()
            .filter(|(_, service)| service.pairing_visible)
            .map(|(full_name, service)| DiscoveryCandidate {
                id: candidate_id(full_name),
                endpoints: service.endpoints.clone(),
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.id.cmp(&right.id));
        candidates
    }

    pub(crate) fn endpoints(&self) -> Vec<SocketAddr> {
        self.endpoints
            .lock()
            .expect("Local Link endpoint mutex poisoned")
            .values()
            .flat_map(|service| service.endpoints.iter().copied())
            .collect()
    }

    pub(crate) fn set_pairing_visible(&mut self, visible: bool) -> AppResult<()> {
        let _ = self.daemon.unregister(&self.full_name);
        let service = service_info(&self.instance_name, &self.host_name, self.port, visible)?;
        self.full_name = service.get_fullname().to_owned();
        self.daemon.register(service).map_err(discovery_error)
    }
}

impl Drop for DiscoveryRuntime {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.daemon.unregister(&self.full_name);
        let _ = self.daemon.stop_browse(SERVICE_TYPE);
        let _ = self.daemon.shutdown();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        self.endpoints
            .lock()
            .expect("Local Link endpoint mutex poisoned")
            .clear();
    }
}

fn service_info(
    instance_name: &str,
    host_name: &str,
    port: u16,
    pairing_visible: bool,
) -> AppResult<ServiceInfo> {
    let version = PROTOCOL_VERSION.to_string();
    let pairing = if pairing_visible { "1" } else { "0" };
    let properties = [("v", version.as_str()), ("pair", pairing)];
    ServiceInfo::new(
        SERVICE_TYPE,
        instance_name,
        host_name,
        "",
        port,
        &properties[..],
    )
    .map(ServiceInfo::enable_addr_auto)
    .map_err(discovery_error)
}

fn candidate_id(full_name: &str) -> String {
    let digest = Sha256::digest(full_name.as_bytes());
    let suffix = digest[..10]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("candidate-{suffix}")
}

fn discovery_error(_error: impl std::fmt::Display) -> AppError {
    AppError::InvalidInput(
        "Local Link could not start private Bonjour discovery; lifecycle recovery will retry."
            .to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advertisement_contains_only_protocol_and_pairing_flags() {
        let info = service_info("ll-random", "ll-random.local.", 42_000, false).unwrap();
        assert_eq!(info.get_property_val_str("v"), Some("3"));
        assert_eq!(info.get_property_val_str("pair"), Some("0"));
        assert_eq!(info.get_properties().iter().count(), 2);
        assert!(!info.get_fullname().contains("Mac"));
        assert!(candidate_id(info.get_fullname()).starts_with("candidate-"));
    }
}
