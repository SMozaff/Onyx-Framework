//! Domain reference types — lightweight pointers to domain objects, carrying
//! just enough context (type discriminant, tenant) for routing and tenancy
//! enforcement without pulling in the full aggregate.

use crate::authority::{DeviceId, OrganizationId, UserId};
use crate::identifiers::ObjectId;
use serde::{Deserialize, Serialize};

/// A lightweight, typed pointer to any domain object, carrying just enough
/// context (id, type discriminant, tenant) for routing without pulling in
/// the full aggregate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DomainObjectRef {
    /// The referenced object's identifier.
    pub id: ObjectId,
    /// The referenced object's type discriminant (e.g. `"mission"`, `"task"`).
    pub r#type: String,
    /// The tenant that owns the referenced object.
    pub organization_id: OrganizationId,
}

/// A lightweight pointer to a user within an organization.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserRef {
    /// The user's identifier.
    pub id: UserId,
    /// The tenant the user belongs to.
    pub organization_id: OrganizationId,
}

/// A lightweight pointer to an organization.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrganizationRef {
    /// The organization's identifier.
    pub id: OrganizationId,
}

/// A lightweight pointer to a device within an organization.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceRef {
    /// The device's identifier.
    pub id: DeviceId,
    /// The tenant the device belongs to.
    pub organization_id: OrganizationId,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identifiers::ObjectId;

    #[test]
    fn domain_object_ref_constructs() {
        let r = DomainObjectRef {
            id: ObjectId::new_random(),
            r#type: "mission".to_string(),
            organization_id: ObjectId::new_random(),
        };
        assert_eq!(r.r#type, "mission");
    }
}
