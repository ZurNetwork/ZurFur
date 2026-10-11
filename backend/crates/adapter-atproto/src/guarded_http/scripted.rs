//! A scripted DNS for tests: fixed answers per name, and a record of every name asked.

use std::{collections::HashMap, net::IpAddr, sync::Mutex};

use async_trait::async_trait;

use super::dns::{AddressLookup, LookupError};

/// What a scripted name answers with.
#[derive(Clone, Debug)]
pub(crate) enum Script {
    /// These addresses.
    Answer(Vec<IpAddr>),
    /// The lookup fails outright.
    Fail,
    /// The lookup never finishes.
    Hang,
}

/// Answers each scripted fully qualified name with its [`Script`]; any other
/// name has no records.
#[derive(Default)]
pub(crate) struct ScriptedLookup {
    scripts: HashMap<String, Script>,
    asked: Mutex<Vec<String>>,
}

impl ScriptedLookup {
    /// Add `fqdn` answering with `script`.
    pub(crate) fn with(mut self, fqdn: &str, script: Script) -> Self {
        self.scripts.insert(fqdn.to_owned(), script);
        self
    }

    /// Every name asked so far, in order.
    pub(crate) fn asked(&self) -> Vec<String> {
        self.asked.lock().expect("lock").clone()
    }
}

#[async_trait]
impl AddressLookup for ScriptedLookup {
    async fn lookup_ip(&self, fqdn: &str) -> Result<Vec<IpAddr>, LookupError> {
        self.asked.lock().expect("lock").push(fqdn.to_owned());
        let script = self.scripts.get(fqdn).cloned();
        match script {
            Some(Script::Answer(addresses)) => Ok(addresses),
            Some(Script::Fail) => Err(LookupError::Failed("scripted failure".into())),
            Some(Script::Hang) => std::future::pending().await,
            None => Err(LookupError::NoRecords),
        }
    }
}
