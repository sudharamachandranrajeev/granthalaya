//! Catalogue passwords. Desktop: the system keyring. Phones: a file in the app's
//! private storage, which other apps cannot read. Never in the library database.

use std::collections::HashMap;
use std::path::PathBuf;

#[cfg(not(mobile))]
const SERVICE: &str = "Granthalaya";

pub struct Secrets {
    #[cfg_attr(not(mobile), allow(dead_code))]
    file: PathBuf,
    /// Why the keyring cannot be used, if it cannot.
    #[cfg_attr(mobile, allow(dead_code))]
    broken: Option<String>,
}

impl Secrets {
    pub fn new(dir: &std::path::Path) -> Self {
        Secrets { file: dir.join("secrets.json"), broken: init().err() }
    }

    pub fn get(&self, key: &str) -> Result<Option<String>, String> {
        #[cfg(mobile)]
        return Ok(self.read_file()?.remove(key));
        #[cfg(not(mobile))]
        match self.entry(key)?.get_password() {
            Ok(p) => Ok(Some(p)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("could not read the password from the system keyring ({e})")),
        }
    }

    pub fn set(&self, key: &str, password: &str) -> Result<(), String> {
        #[cfg(mobile)]
        {
            let mut all = self.read_file()?;
            all.insert(key.into(), password.into());
            return self.write_file(&all);
        }
        #[cfg(not(mobile))]
        self.entry(key)?.set_password(password).map_err(|e| format!("could not save the password in the system keyring ({e})"))
    }

    pub fn remove(&self, key: &str) -> Result<(), String> {
        #[cfg(mobile)]
        {
            let mut all = self.read_file()?;
            all.remove(key);
            return self.write_file(&all);
        }
        #[cfg(not(mobile))]
        match self.entry(key)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("could not remove the password from the system keyring ({e})")),
        }
    }

    #[cfg(not(mobile))]
    fn entry(&self, key: &str) -> Result<keyring_core::Entry, String> {
        if let Some(e) = &self.broken {
            return Err(format!("the system keyring is not available, so logins cannot be kept ({e})"));
        }
        keyring_core::Entry::new(SERVICE, key).map_err(|e| e.to_string())
    }

    #[cfg_attr(not(mobile), allow(dead_code))]
    fn read_file(&self) -> Result<HashMap<String, String>, String> {
        match std::fs::read(&self.file) {
            Ok(b) => serde_json::from_slice(&b).map_err(|e| e.to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
            Err(e) => Err(e.to_string()),
        }
    }

    #[cfg_attr(not(mobile), allow(dead_code))]
    fn write_file(&self, all: &HashMap<String, String>) -> Result<(), String> {
        let tmp = self.file.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_vec(all).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
        }
        std::fs::rename(&tmp, &self.file).map_err(|e| e.to_string())
    }
}

fn init() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    keyring_core::set_default_store(zbus_secret_service_keyring_store::Store::new().map_err(|e| e.to_string())?);
    #[cfg(windows)]
    keyring_core::set_default_store(windows_native_keyring_store::Store::new().map_err(|e| e.to_string())?);
    #[cfg(target_os = "macos")]
    keyring_core::set_default_store(apple_native_keyring_store::keychain::Store::new().map_err(|e| e.to_string())?);
    Ok(())
}
