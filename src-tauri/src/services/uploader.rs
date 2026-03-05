use std::path::Path;

use sha1::{Digest, Sha1};

use crate::error::AppError;

pub async fn upload_to_b2(
    file_path: &Path,
    upload_url: &str,
    authorization_token: &str,
    file_name: &str,
) -> Result<(), AppError> {
    let data = tokio::fs::read(file_path).await?;
    let sha1_hex = hex_sha1(&data);

    let client = reqwest::Client::new();
    let resp = client
        .post(upload_url)
        .header("Authorization", authorization_token)
        .header("X-Bz-File-Name", file_name)
        .header("Content-Type", "audio/mp4")
        .header("Content-Length", data.len().to_string())
        .header("X-Bz-Content-Sha1", &sha1_hex)
        .body(data)
        .send()
        .await?;

    let status = resp.status().as_u16();
    if !(200..=299).contains(&status) {
        return Err(AppError::UploadFailed(status));
    }

    Ok(())
}

fn hex_sha1(data: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(data);
    let result = hasher.finalize();
    result.iter().map(|b| format!("{b:02x}")).collect()
}
