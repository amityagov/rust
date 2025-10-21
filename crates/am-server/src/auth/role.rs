use crate::auth::AuthenticatedUser;
use crate::error::ApplicationError;

pub trait RoleContainer {
    type RoleType: Role + Eq;

    fn role(&self) -> Option<Self::RoleType>;

    fn require_role<R: Role + Eq + PartialEq<Self::RoleType>>(
        &self,
        role: R,
    ) -> Result<(), ApplicationError> {
        if let Some(r) = self.role() {
            if role == r {
                Ok(())
            } else {
                Err(ApplicationError::Forbidden)
            }
        } else {
            Err(ApplicationError::Forbidden)
        }
    }
}

impl RoleContainer for AuthenticatedUser {
    type RoleType = String;

    fn role(&self) -> Option<Self::RoleType> {
        self.role.clone()
    }
}

pub trait Role {}

impl Role for String {}

impl Role for &str {}
