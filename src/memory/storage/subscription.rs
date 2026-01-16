use super::super::storage::Status as OperationStatus;
use super::super::subscription::Operations as SubscriptionOp;
use super::super::subscription::Subscriptions;

pub trait Operations<S>
where
    S: Send + Sync + Clone + 'static + PartialEq,
{
    fn find_by_id(&self, id: &uuid::Uuid) -> Option<&S>;
    fn find_by_id_mut(&mut self, id: &uuid::Uuid) -> Option<&mut S>;
    fn find_by_referral_code(&self, code: &str) -> Option<&S>;
    fn all(&self) -> Vec<S>;
    fn add(&mut self, new_subscription: S) -> OperationStatus;
    fn delete(&mut self, id: &uuid::Uuid);
    fn update(&mut self, subscription: S);
    fn count_invited_by(&self, referral_code: &str) -> usize;
}

impl<S> Operations<S> for Subscriptions<S>
where
    S: SubscriptionOp + Send + Sync + Clone + 'static + PartialEq + SubscriptionOp,
{
    fn count_invited_by(&self, referral_code: &str) -> usize {
        self.values()
            .inspect(|s| {
                log::debug!(
                    "cmp: referred_by={:?}  target={:?}",
                    s.referred_by(),
                    referral_code
                );
            })
            .filter(|s| s.referred_by().as_deref() == Some(referral_code))
            .count()
    }

    fn find_by_id(&self, id: &uuid::Uuid) -> Option<&S> {
        self.get(id)
    }
    fn find_by_id_mut(&mut self, id: &uuid::Uuid) -> Option<&mut S> {
        self.get_mut(id)
    }

    fn find_by_referral_code(&self, code: &str) -> Option<&S> {
        self.values()
            .find(|s| s.referral_code() == code.to_string())
    }

    fn all(&self) -> Vec<S> {
        self.values().cloned().collect()
    }

    fn add(&mut self, new_subscription: S) -> OperationStatus {
        let id = new_subscription.id();

        match self.get_mut(&id) {
            Some(existing) => {
                if existing == &new_subscription {
                    OperationStatus::AlreadyExist(id)
                } else {
                    *existing = new_subscription;
                    OperationStatus::Updated(id)
                }
            }
            None => {
                self.insert(id, new_subscription);
                OperationStatus::Ok(id)
            }
        }
    }

    fn delete(&mut self, id: &uuid::Uuid) {
        self.remove(id);
    }

    fn update(&mut self, subscription: S) {
        self.insert(subscription.id(), subscription);
    }
}
