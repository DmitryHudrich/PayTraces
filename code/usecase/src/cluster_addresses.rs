use domain::entity::{ClusterEvidence, ClusteringHeuristic, Entity, EntityCategory, RiskScore};
use domain::error::DomainResult;
use domain::ports::{EntityRepository, TransferRepository};
use domain::primitives::{Address, Confidence};

pub struct ClusterAddressesUseCase<R, E> {
    transfers: R,
    entities: E,
}

impl<R: TransferRepository, E: EntityRepository> ClusterAddressesUseCase<R, E> {
    pub fn new(transfers: R, entities: E) -> Self {
        Self {
            transfers,
            entities,
        }
    }

    pub async fn deposit_reuse_cluster(
        &self,
        deposit_addr: &Address,
    ) -> DomainResult<Option<ClusterEvidence>> {
        let incoming = self.transfers.find_incoming(deposit_addr, None).await?;

        if incoming.len() < 3 {
            return Ok(None);
        }

        let senders: Vec<Address> = {
            let mut v: Vec<Address> = incoming.into_iter().map(|t| t.from().clone()).collect();
            v.sort_by(|a, b| a.bytes().cmp(b.bytes()));
            v.dedup();
            v
        };

        if senders.len() < 2 {
            return Ok(None);
        }

        Ok(Some(ClusterEvidence::new(
            senders,
            ClusteringHeuristic::DepositAddressReuse,
            Confidence::MEDIUM,
            Some(format!(
                "All senders route to deposit address {}",
                deposit_addr
            )),
        )))
    }

    pub async fn detect_peeling_chain(
        &self,
        addr: &Address,
    ) -> DomainResult<Option<ClusterEvidence>> {
        let incoming = self.transfers.find_incoming(addr, None).await?;
        let outgoing = self.transfers.find_outgoing(addr, None).await?;

        if incoming.is_empty() || outgoing.is_empty() {
            return Ok(None);
        }

        let in_sum = incoming
            .iter()
            .fold(None::<domain::primitives::Amount>, |acc, t| {
                Some(acc.map(|a| a + t.amount()).unwrap_or(t.amount()))
            });
        let out_sum = outgoing
            .iter()
            .fold(None::<domain::primitives::Amount>, |acc, t| {
                Some(acc.map(|a| a + t.amount()).unwrap_or(t.amount()))
            });

        let (Some(in_total), Some(out_total)) = (in_sum, out_sum) else {
            return Ok(None);
        };

        let retained = if out_total.raw() <= in_total.raw() {
            in_total - out_total
        } else {
            return Ok(None);
        };

        let retained_ratio = retained.ratio_of(&in_total);

        if retained_ratio > domain::primitives::Ratio::from_percent(5) {
            return Ok(None);
        }

        let chain_addrs: Vec<Address> = outgoing.into_iter().map(|t| t.to().clone()).collect();

        Ok(Some(ClusterEvidence::new(
            chain_addrs,
            ClusteringHeuristic::PeelingChain,
            Confidence::HIGH,
            Some(format!(
                "Address retains only {:.1}% of inflow",
                retained_ratio.as_f64() * 100.0
            )),
        )))
    }

    pub async fn save_cluster(
        &self,
        evidence: ClusterEvidence,
        category: EntityCategory,
    ) -> DomainResult<()> {
        let mut entity = Entity::new(category, RiskScore::MEDIUM);
        for addr in evidence.addresses() {
            entity.add_address(addr.clone());
        }
        self.entities.save(&entity).await
    }
}

