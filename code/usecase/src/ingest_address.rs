use domain::error::DomainResult;
use domain::ports::{BlockRange, ChainSource, TransferRepository};
use domain::primitives::Address;

use crate::addr_hex;

pub struct IngestAddressUseCase<S, R> {
    source: S,
    repo: R,
}

impl<S: ChainSource, R: TransferRepository> IngestAddressUseCase<S, R> {
    pub fn new(source: S, repo: R) -> Self {
        Self { source, repo }
    }

    pub async fn execute(&self, addr: &Address, range: BlockRange) -> DomainResult<usize> {
        tracing::info!(
            address = %addr_hex(addr),
            from_block = range.from_height(),
            to_block = range.to_height(),
            "ingest started"
        );

        let transfers = self.source.transfers_for_address(addr, range).await?;
        let count = transfers.len();

        tracing::info!(address = %addr_hex(addr), count, "fetched transfers, saving");

        self.repo.save(&transfers).await?;

        tracing::info!(address = %addr_hex(addr), count, "ingest complete");
        Ok(count)
    }
}

