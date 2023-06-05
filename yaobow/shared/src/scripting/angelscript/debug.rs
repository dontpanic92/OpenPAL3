use std::io::BufReader;

use anyhow::Context;
use interprocess::local_socket::LocalSocketStream;

pub(super) fn create_client() -> anyhow::Result<BufReader<LocalSocketStream>> {
    LocalSocketStream::connect("asdebug-localsocket")
        .context("Connect failed")
        .map(BufReader::new)
}
