use domain::entity::{EntityCategory, SanctionList};
use domain::error::DomainResult;
use domain::ports::EntityRepository;
use domain::primitives::Address;

#[derive(Debug, Clone)]
pub struct SanctionsCheckResult {
    pub address: Address,
    pub is_sanctioned: bool,
    pub sanction_list: Option<SanctionList>,
    pub label: Option<String>,
}

pub struct CheckSanctionsUseCase<E> {
    entities: E,
}

impl<E: EntityRepository> CheckSanctionsUseCase<E> {
    pub fn new(entities: E) -> Self {
        Self { entities }
    }

    pub async fn execute(&self, addr: &Address) -> DomainResult<SanctionsCheckResult> {
        tracing::debug!(address = %super::addr_hex(addr), "sanctions check");
        let entity = self.entities.find_by_address(addr).await?;

        let (is_sanctioned, sanction_list, label) = match entity {
            Some(e) => {
                let list = if let EntityCategory::Sanctioned { sanction_list } = e.category() {
                    Some(sanction_list.clone())
                } else {
                    None
                };
                let label = e.label().map(|l| l.name().to_string());
                (list.is_some(), list, label)
            }
            None => (false, None, None),
        };

        tracing::debug!(
            address = %super::addr_hex(addr),
            is_sanctioned,
            "sanctions check result"
        );
        Ok(SanctionsCheckResult {
            address: addr.clone(),
            is_sanctioned,
            sanction_list,
            label,
        })
    }

    pub async fn check_batch(
        &self,
        addrs: &[Address],
    ) -> DomainResult<Vec<SanctionsCheckResult>> {
        let mut results = Vec::with_capacity(addrs.len());
        for addr in addrs {
            results.push(self.execute(addr).await?);
        }
        Ok(results)
    }
}

