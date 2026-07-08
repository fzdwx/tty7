use std::path::PathBuf;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

use super::{Session, SessionPane, SessionTab, SessionWorkspace};

#[derive(Default, Deserialize)]
#[serde(default)]
struct SessionTabWire {
    name: Option<String>,
    kind: Option<String>,
    pane: Option<SessionPane>,
    path: Option<PathBuf>,
}

impl<'de> Deserialize<'de> for SessionTab {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = SessionTabWire::deserialize(deserializer)?;
        match wire.kind.as_deref() {
            None | Some("Terminal") => {
                let pane = wire
                    .pane
                    .ok_or_else(|| D::Error::custom("terminal tab missing pane"))?;
                Ok(Self::terminal(wire.name, pane))
            }
            Some("Preview") => {
                let path = wire
                    .path
                    .ok_or_else(|| D::Error::custom("preview tab missing path"))?;
                Ok(Self::preview(wire.name, path))
            }
            Some(kind) => Err(D::Error::custom(format!("unknown tab kind {kind}"))),
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct SessionWire {
    active_workspace: usize,
    workspaces: Vec<SessionWorkspace>,
    active: Option<usize>,
    tabs: Option<Vec<SessionTab>>,
}

impl<'de> Deserialize<'de> for Session {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = SessionWire::deserialize(deserializer)?;
        if let Some(tabs) = wire.tabs {
            return Ok(Self::from_tabs(wire.active.unwrap_or(0), tabs));
        }

        let mut session = Self {
            active_workspace: wire.active_workspace,
            workspaces: wire.workspaces,
        };
        session.normalize();
        Ok(session)
    }
}
