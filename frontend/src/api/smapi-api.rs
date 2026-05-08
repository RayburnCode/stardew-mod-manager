use std::collections::HashMap;

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;
use time::OffsetDateTime;

use crate::api::mod_manager::{InstalledMod, is_update_available};

const SMAPI_MODS_ENDPOINT: &str = "https://smapi.io/api/v3.0/mods";

#[derive(Debug, Serialize)]
struct SmapiRequest {
  mods: Vec<SmapiRequestMod>,
  #[serde(rename = "includeExtendedMetadata")]
  include_extended_metadata: bool,
}

#[derive(Debug, Serialize)]
struct SmapiRequestMod {
  id: String,
  #[serde(rename = "installedVersion")]
  installed_version: String,
  #[serde(rename = "updateKeys")]
  update_keys: Vec<String>,
}

pub async fn fetch_latest_versions(mods: &[InstalledMod]) -> Result<HashMap<String, SmapiUpdateInfo>> {
  if mods.is_empty() {
    return Ok(HashMap::new());
  }

  let body = SmapiRequest {
    mods: mods
      .iter()
      .map(|m| SmapiRequestMod {
        id: m.manifest.unique_id.clone(),
        installed_version: m.manifest.version.clone(),
        update_keys: m.manifest.update_keys.clone(),
      })
      .collect(),
    include_extended_metadata: true,
  };

  let mut installed_by_id: HashMap<String, &str> = HashMap::new();
  for m in mods {
    installed_by_id.insert(m.manifest.unique_id.clone(), m.manifest.version.as_str());
  }

  let http = reqwest::Client::builder()
    .user_agent("StardewModManager/0.1")
    .http1_only()
    .timeout(std::time::Duration::from_secs(20))
    .build()
    .context("Failed to build SMAPI HTTP client")?;

  let resp = http
    .post(SMAPI_MODS_ENDPOINT)
    .json(&body)
    .send()
    .await
    .context("SMAPI request failed")?
    .error_for_status()
    .context("SMAPI API returned an error status")?;

  let payload: Value = resp
    .json()
    .await
    .context("Failed to parse SMAPI response JSON")?;

  let mut latest_by_id: HashMap<String, SmapiUpdateInfo> = HashMap::new();

  for entry in response_entries(&payload) {
    let Some(id) = find_string(entry, &[vec!["id"], vec!["uniqueID"], vec!["uniqueId"]]) else {
      continue;
    };

    let Some(latest) = find_string(
      entry,
      &[
        vec!["suggestedUpdate", "version"],
        vec!["main", "version"],
        vec!["latestVersion"],
        vec!["version"],
        vec!["latest", "version"],
      ],
    ) else {
      continue;
    };

    let Some(installed) = installed_by_id.get(&id) else {
      continue;
    };

    if is_update_available(installed, &latest) {
      latest_by_id.insert(
        id,
        SmapiUpdateInfo {
          latest_version: latest,
          updated_timestamp: find_timestamp(
            entry,
            &[
              vec!["suggestedUpdate", "date"],
              vec!["suggestedUpdate", "updatedOn"],
              vec!["main", "date"],
              vec!["main", "updatedOn"],
              vec!["latest", "date"],
              vec!["updatedOn"],
              vec!["date"],
            ],
          ),
        },
      );
    }
  }

  Ok(latest_by_id)
}

#[derive(Debug, Clone)]
pub struct SmapiUpdateInfo {
  pub latest_version: String,
  pub updated_timestamp: Option<u64>,
}

fn response_entries(payload: &Value) -> Vec<&Value> {
  if let Some(arr) = payload.as_array() {
    return arr.iter().collect();
  }

  payload
    .get("mods")
    .and_then(Value::as_array)
    .map(|arr| arr.iter().collect())
    .unwrap_or_default()
}

fn find_string(value: &Value, paths: &[Vec<&str>]) -> Option<String> {
  for path in paths {
    let mut current = value;
    let mut found = true;

    for segment in path {
      let Some(next) = current.get(*segment) else {
        found = false;
        break;
      };
      current = next;
    }

    if found {
      if let Some(s) = current.as_str() {
        if !s.trim().is_empty() {
          return Some(s.to_string());
        }
      }
    }
  }

  None
}

fn find_timestamp(value: &Value, paths: &[Vec<&str>]) -> Option<u64> {
  for path in paths {
    let mut current = value;
    let mut found = true;

    for segment in path {
      let Some(next) = current.get(*segment) else {
        found = false;
        break;
      };
      current = next;
    }

    if !found {
      continue;
    }

    if let Some(num) = current.as_u64() {
      return Some(normalize_unix(num));
    }

    if let Some(s) = current.as_str() {
      let trimmed = s.trim();
      if trimmed.is_empty() {
        continue;
      }

      if let Ok(num) = trimmed.parse::<u64>() {
        return Some(normalize_unix(num));
      }

      if let Ok(dt) = OffsetDateTime::parse(trimmed, &time::format_description::well_known::Rfc3339) {
        return Some(dt.unix_timestamp().max(0) as u64);
      }
    }
  }

  None
}

fn normalize_unix(ts: u64) -> u64 {
  // Some APIs return milliseconds. Convert to seconds if needed.
  if ts > 1_000_000_000_000 {
    ts / 1_000
  } else {
    ts
  }
}