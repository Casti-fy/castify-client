use crate::error::AppError;

const SERVICE: &str = "com.castify.app";
const ACCOUNT: &str = "jwt_token";

pub fn save_token(token: &str) -> Result<(), AppError> {
    let entry = keyring::Entry::new(SERVICE, ACCOUNT)
        .map_err(|e| AppError::Keychain(e.to_string()))?;
    entry
        .set_password(token)
        .map_err(|e| AppError::Keychain(e.to_string()))
}

pub fn get_token() -> Result<String, AppError> {
    let entry = keyring::Entry::new(SERVICE, ACCOUNT)
        .map_err(|e| AppError::Keychain(e.to_string()))?;
    entry
        .get_password()
        .map_err(|e| AppError::Keychain(e.to_string()))
}

pub fn delete_token() -> Result<(), AppError> {
    let entry = keyring::Entry::new(SERVICE, ACCOUNT)
        .map_err(|e| AppError::Keychain(e.to_string()))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::Keychain(e.to_string())),
    }
}
