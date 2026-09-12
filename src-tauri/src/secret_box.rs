use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const MASTER_LEN: usize = 32;
pub const NONCE_LEN: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedFile {
    pub version: u32,
    pub protect: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrapped_key: Option<String>,
    pub nonce: String,
    pub ciphertext: String,
}

pub fn random_master() -> Result<[u8; MASTER_LEN], String> {
    let mut key = [0_u8; MASTER_LEN];
    getrandom::getrandom(&mut key).map_err(|_| "无法生成加密密钥。".to_string())?;
    Ok(key)
}

pub fn seal(master: &[u8; MASTER_LEN], plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    let cipher = Aes256Gcm::new_from_slice(master).map_err(|_| "加密密钥无效。".to_string())?;
    let mut nonce_bytes = [0_u8; NONCE_LEN];
    getrandom::getrandom(&mut nonce_bytes).map_err(|_| "无法生成加密随机数。".to_string())?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| "加密失败。".to_string())?;
    Ok((nonce_bytes.to_vec(), ciphertext))
}

pub fn open(master: &[u8; MASTER_LEN], nonce: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    if nonce.len() != NONCE_LEN {
        return Err("加密数据损坏。".into());
    }
    let cipher = Aes256Gcm::new_from_slice(master).map_err(|_| "加密密钥无效。".to_string())?;
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| "无法解密凭据保险箱。本机用户或密钥文件不匹配。".to_string())
}

pub fn encode(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

pub fn decode(text: &str) -> Result<Vec<u8>, String> {
    STANDARD
        .decode(text.trim())
        .map_err(|_| "加密数据无法解码。".to_string())
}

pub fn wrap_master(master: &[u8; MASTER_LEN]) -> Result<(String, Option<Vec<u8>>), String> {
    #[cfg(windows)]
    {
        return Ok(("dpapi".into(), Some(dpapi_protect(master)?)));
    }
    #[cfg(not(windows))]
    {
        let _ = master;
        Ok(("file".into(), None))
    }
}

pub fn unwrap_master(
    protect: &str,
    wrapped: Option<&[u8]>,
    master_file: &Path,
) -> Result<[u8; MASTER_LEN], String> {
    match protect {
        "dpapi" => {
            #[cfg(windows)]
            {
                let blob = wrapped.ok_or_else(|| "保险箱缺少 Windows 保护密钥。".to_string())?;
                return bytes_to_master(&dpapi_unprotect(blob)?);
            }
            #[cfg(not(windows))]
            {
                let _ = wrapped;
                return Err("这个保险箱是用 Windows 用户凭据加密的，无法在当前系统打开。".into());
            }
        }
        "file" => read_master_file(master_file),
        other => Err(format!("不支持的保护方式：{other}")),
    }
}

#[cfg(not(windows))]
pub fn ensure_master_file(path: &Path) -> Result<[u8; MASTER_LEN], String> {
    if path.exists() {
        return read_master_file(path);
    }
    let key = random_master()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("无法创建保险箱目录：{err}"))?;
    }
    fs::write(path, key).map_err(|err| format!("无法写入主密钥：{err}"))?;
    restrict_file(path);
    Ok(key)
}

fn read_master_file(path: &Path) -> Result<[u8; MASTER_LEN], String> {
    let bytes = fs::read(path).map_err(|_| "找不到保险箱主密钥，无法解密已保存的 Key。".to_string())?;
    bytes_to_master(&bytes)
}

fn bytes_to_master(bytes: &[u8]) -> Result<[u8; MASTER_LEN], String> {
    bytes
        .try_into()
        .map_err(|_| "主密钥长度不正确。".to_string())
}

#[cfg(not(windows))]
fn restrict_file(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    let _ = path;
}

#[cfg(windows)]
fn dpapi_protect(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB};
    let mut input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let ok = unsafe {
        CryptProtectData(
            &mut input,
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            &mut output,
        )
    };
    if ok == 0 {
        return Err("无法用 Windows 用户凭据加密 Key。".into());
    }
    let slice = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) };
    let out = slice.to_vec();
    unsafe {
        let _ = LocalFree(output.pbData as _);
    }
    Ok(out)
}

#[cfg(windows)]
fn dpapi_unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};
    let mut input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let ok = unsafe {
        CryptUnprotectData(
            &mut input,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            &mut output,
        )
    };
    if ok == 0 {
        return Err("无法用 Windows 用户凭据解密 Key。请确认是同一台电脑、同一个 Windows 用户。".into());
    }
    let slice = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) };
    let out = slice.to_vec();
    unsafe {
        let _ = LocalFree(output.pbData as _);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_aes() {
        let master = random_master().unwrap();
        let (nonce, ct) = seal(&master, b"sk-secret-value").unwrap();
        let plain = open(&master, &nonce, &ct).unwrap();
        assert_eq!(plain, b"sk-secret-value");
    }

    #[test]
    fn wrong_master_fails() {
        let master = random_master().unwrap();
        let other = random_master().unwrap();
        let (nonce, ct) = seal(&master, b"sk-secret-value").unwrap();
        assert!(open(&other, &nonce, &ct).is_err());
    }

    #[test]
    fn ciphertext_does_not_contain_plaintext() {
        let master = random_master().unwrap();
        let secret = b"sk-abcdefghijklmnop-unique";
        let (_nonce, ct) = seal(&master, secret).unwrap();
        assert!(!ct.windows(secret.len()).any(|w| w == secret));
    }
}
