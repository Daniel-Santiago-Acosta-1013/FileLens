//! Utilidades dependientes de Unix para mostrar permisos detallados.

#[cfg(unix)]
pub fn owner_name(metadata: &std::fs::Metadata) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    use users::get_user_by_uid;

    let uid = metadata.uid();
    get_user_by_uid(uid).map(|user| user.name().to_string_lossy().into_owned())
}

#[cfg(unix)]
pub fn group_name(metadata: &std::fs::Metadata) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    use users::get_group_by_gid;

    let gid = metadata.gid();
    get_group_by_gid(gid).map(|group| group.name().to_string_lossy().into_owned())
}

#[cfg(unix)]
pub fn format_unix_permissions(mode: u32) -> String {
    const SYMBOLS: [&str; 8] = ["---", "--x", "-w-", "-wx", "r--", "r-x", "rw-", "rwx"];

    let user = SYMBOLS[((mode >> 6) & 0o7) as usize];
    let group = SYMBOLS[((mode >> 3) & 0o7) as usize];
    let other = SYMBOLS[(mode & 0o7) as usize];

    format!("{}{}{}", user, group, other)
}
