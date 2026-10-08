//! Stable references to domain objects.
//! Source: Handover Document §1 "domain_references" (lines 159-188).

use crate::identifiers::{DeviceId, ObjectId, OrganizationId, UserId};
use serde::{Deserialize, Serialize};

/// Stable reference to any domain object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainObjectRef {
    pub id: ObjectId,
    #[serde(rename = "type")]
    pub object_type: String,
    pub organization_id: OrganizationId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserRef {
    pub id: UserId,
    pub organization_id: OrganizationId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganizationRef {
    pub id: OrganizationId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceRef {
    pub id: DeviceId,
    pub organization_id: OrganizationId,
}
