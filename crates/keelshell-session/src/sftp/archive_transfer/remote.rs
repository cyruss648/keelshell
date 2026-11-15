//! Strict remote observations and EXCL staging with standard SFTP publication.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Stamp {
    mode: u32,
    bytes: Option<u64>,
    modified: Option<u32>,
    owner: (Option<u32>, Option<u32>),
}

impl Stamp {
    fn directory(attrs: FileAttributes) -> Result<Self> {
        let mode = attrs
            .permissions
            .filter(|mode| mode & 0o170000 == 0o040000)
            .ok_or(SessionError::Invalid(
                "archive remote ancestor is a link or has no directory type",
            ))?;
        Ok(Self {
            mode,
            bytes: None,
            modified: None,
            owner: (attrs.uid, attrs.gid),
        })
    }

    fn file(attrs: FileAttributes) -> Result<Self> {
        let mode = attrs
            .permissions
            .filter(|mode| mode & 0o170000 == 0o100000)
            .ok_or(SessionError::Invalid(
                "archive remote source is a link or not a regular file",
            ))?;
        let bytes = attrs
            .size
            .ok_or(SessionError::Invalid("archive remote file size is unknown"))?;
        if bytes > MAX_ARCHIVE_BYTES {
            return Err(SessionError::OutputLimit(MAX_ARCHIVE_BYTES as usize));
        }
        Ok(Self {
            mode,
            bytes: Some(bytes),
            modified: attrs.mtime,
            owner: (attrs.uid, attrs.gid),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Observation {
    parents: Vec<(String, Stamp)>,
    pub(super) leaf: Option<Stamp>,
}

async fn parents(
    raw: &RawSftpSession,
    path: &str,
    authority: &Authority<'_>,
) -> Result<Vec<(String, Stamp)>> {
    let parent = directory::remote_parent(path)?;
    let mut paths = vec!["/".to_owned()];
    if parent != "/" {
        let mut current = String::new();
        for component in parent[1..].split('/') {
            current.push('/');
            current.push_str(component);
            paths.push(current.clone());
        }
    }
    let mut result = Vec::with_capacity(paths.len());
    for path in paths {
        let attrs = authority.packet(raw.lstat(&path)).await?.attrs;
        let stamp = Stamp::directory(attrs)?;
        let resolved = authority.packet(raw.realpath(&path)).await?;
        if resolved.files.len() != 1 || resolved.files[0].filename != path {
            return Err(SessionError::Invalid(
                "archive rejects remote canonical path aliases",
            ));
        }
        result.push((path, stamp));
    }
    Ok(result)
}

async fn leaf(
    raw: &RawSftpSession,
    path: &str,
    authority: &Authority<'_>,
) -> Result<Option<FileAttributes>> {
    authority
        .wait(async {
            match raw.lstat(path).await {
                Ok(packet) => Ok(Some(packet.attrs)),
                Err(russh_sftp::client::error::Error::Status(status))
                    if status.status_code == StatusCode::NoSuchFile =>
                {
                    Ok(None)
                }
                Err(error) => Err(sftp_error(error)),
            }
        })
        .await
}

async fn absent(raw: &RawSftpSession, path: &str, authority: &Authority<'_>) -> Result<()> {
    if leaf(raw, path, authority).await?.is_some() {
        return Err(SessionError::Invalid(
            "archive remote destination already exists",
        ));
    }
    Ok(())
}

pub(super) async fn observe(
    raw: &RawSftpSession,
    request: &ArchiveTransferRequest,
    authority: &Authority<'_>,
) -> Result<Observation> {
    let parents = parents(raw, request.remote(), authority).await?;
    let attrs = leaf(raw, request.remote(), authority).await?;
    let leaf = match request {
        ArchiveTransferRequest::PackUpload { .. } => {
            if attrs.is_some() {
                return Err(SessionError::Invalid(
                    "archive remote destination already exists",
                ));
            }
            None
        }
        ArchiveTransferRequest::DownloadExtract { .. } => Some(Stamp::file(
            attrs.ok_or(SessionError::Invalid("archive remote source is missing"))?,
        )?),
    };
    Ok(Observation { parents, leaf })
}

pub(super) async fn read(
    raw: &RawSftpSession,
    path: &str,
    authority: &Authority<'_>,
) -> Result<(Stamp, Vec<u8>)> {
    let chain = parents(raw, path, authority).await?;
    let before = Stamp::file(
        leaf(raw, path, authority)
            .await?
            .ok_or(SessionError::Invalid("archive source is missing"))?,
    )?;
    let handle = authority
        .packet(raw.open(path, OpenFlags::READ, FileAttributes::empty()))
        .await?
        .handle;
    let result = async {
        if Stamp::file(authority.packet(raw.fstat(&handle)).await?.attrs)? != before {
            return Err(changed());
        }
        let mut bytes = Vec::new();
        loop {
            let remaining = MAX_ARCHIVE_BYTES
                .saturating_sub(bytes.len() as u64)
                .saturating_add(1);
            let length = remaining.min(64 * 1024) as u32;
            let packet = authority
                .wait(async {
                    match raw.read(&handle, bytes.len() as u64, length).await {
                        Ok(packet) => Ok(Some(packet.data)),
                        Err(russh_sftp::client::error::Error::Status(status))
                            if status.status_code == StatusCode::Eof =>
                        {
                            Ok(None)
                        }
                        Err(error) => Err(sftp_error(error)),
                    }
                })
                .await?;
            let Some(packet) = packet else { break };
            if packet.is_empty() || packet.len() > length as usize {
                return Err(SessionError::Invalid("invalid archive SFTP read length"));
            }
            bytes.extend_from_slice(&packet);
            if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
                return Err(SessionError::OutputLimit(MAX_ARCHIVE_BYTES as usize));
            }
        }
        if before.bytes != Some(bytes.len() as u64)
            || Stamp::file(authority.packet(raw.fstat(&handle)).await?.attrs)? != before
            || Stamp::file(leaf(raw, path, authority).await?.ok_or_else(changed)?)? != before
            || parents(raw, path, authority).await? != chain
        {
            return Err(changed());
        }
        Ok(bytes)
    }
    .await;
    // CLOSE is read-only here, but it still requires current human authority.
    let close = authority.packet(raw.close(handle)).await;
    let bytes = result?;
    close?;
    Ok((before, bytes))
}

pub(super) async fn publish(
    sftp: &SftpSession,
    plan: &ArchiveTransferPlan,
    raw: &RawSftpSession,
    destination: &str,
    scope: &FileMutationScope<'_>,
    authority: &Authority<'_>,
) -> Result<()> {
    let (parent, name) = destination.rsplit_once('/').ok_or(SessionError::Worker)?;
    let temporary = format!(
        "{parent}/.{name}.keelshell-archive-{}",
        uuid::Uuid::new_v4()
    );
    directory::validate_remote_root(&temporary)?;
    let claim = authority
        .wait(queue::remote_claim(sftp, &temporary, true, true))
        .await?;
    let ticket = scope.ticket()?.add_temporary(claim)?;
    let mut attrs = FileAttributes::empty();
    attrs.permissions = Some(0o600);
    let created = AtomicBool::new(false);
    let opened = authority
        .wait(async {
            let result = scope
                .remote(raw.open(
                    &temporary,
                    OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE,
                    attrs,
                ))
                .await;
            if result.is_ok() {
                created.store(true, Ordering::Release);
            }
            result
        })
        .await;
    let handle = match opened {
        Ok(packet) => packet.handle,
        Err(error) => {
            ticket
                .finish(!scope.pending.load(Ordering::Acquire) && !created.load(Ordering::Acquire));
            return Err(error);
        }
    };
    for (index, chunk) in plan.reviewed.encoded.chunks(64 * 1024).enumerate() {
        authority
            .mutate(
                scope,
                raw.write(&handle, (index * 64 * 1024) as u64, chunk.to_vec()),
            )
            .await?;
    }
    authority.mutate(scope, raw.close(handle)).await?;
    let (stamp, bytes) = read(raw, &temporary, authority).await?;
    if stamp.mode & 0o7777 != 0o600 || bytes != plan.reviewed.encoded {
        return Err(SessionError::UnverifiedMutation(
            "archive private upload readback differs",
        ));
    }
    // Re-read the local manifest and every source byte after staging too. A new
    // destination must still be absent at the final publication boundary.
    sftp.revalidate_archive(plan, raw, authority).await?;
    if parents(raw, destination, authority).await? != plan.reviewed.remote.parents {
        return Err(changed());
    }
    let (current, bytes) = read(raw, &temporary, authority).await?;
    if current != stamp || bytes != plan.reviewed.encoded {
        return Err(SessionError::UnverifiedMutation(
            "archive private upload changed before publication",
        ));
    }
    absent(raw, destination, authority).await?;
    // RawSftpSession::rename sends SSH_FXP_RENAME. The POSIX replacement
    // extension is intentionally never used, even when the server advertises it.
    authority
        .mutate(scope, raw.rename(&temporary, destination))
        .await?;
    let (published, bytes) = read(raw, destination, authority).await?;
    if published.mode & 0o7777 != 0o600
        || bytes != plan.reviewed.encoded
        || parents(raw, destination, authority).await? != plan.reviewed.remote.parents
    {
        return Err(SessionError::UnverifiedMutation(
            "archive published upload readback differs",
        ));
    }
    absent(raw, &temporary, authority).await?;
    authority.check()?;
    ticket.finish(true);
    Ok(())
}
