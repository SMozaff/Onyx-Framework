The ONYX authentication/identity model should be:

```
ALL-FATHER
  │
  │ Google account
  │ so.muzaff@gmail.com
  │
  ▼
Supabase authentication
  │
  │ authenticated All-Father identity
  ▼
ONYX
  │
  ├── Creates Organizations
  │
  ├── Creates/controls Admin accounts
  │       ├── username + password
  │       └── email + password
  │
  └── Admins manage Staff accounts
          ├── register staff
          ├── remove staff
          └── manage staff access
```

So there are actually three distinct authentication/authorization layers.

**1. All-Father authentication**

The All-Father does not use an ONYX username/password.

The All-Father signs in with the Google identity:

`so.muzaff@gmail.com`

Supabase authenticates that Google identity. ONYX then verifies the Supabase identity and recognizes that specific verified identity as the All-Father authority.

The existing code already implements the important part of this model: Supabase verifies the token, retrieves the verified primary email, and compares it with the designated All-Father identity.

**2. Organization/Admin authentication**

The All-Father creates organizations and provisions their administrators.

Admins are ONYX-managed identities. They can authenticate using either:

```
username + password
```

or:

```
email + password
```

Their authorization comes from the ONYX organization/user records, not from the All-Father's Supabase Google identity.

This means we should retain the ONYX password authentication mechanism for Admins. The current Worker migration already has Argon2id verification implemented, so we do not need to invent a weaker password scheme.

**3. Staff lifecycle**

Staff accounts are not self-service accounts.

An administrator creates/registers them, and administrators can remove/deactivate them.

Therefore:

```
Staff cannot independently create an ONYX account.
Staff cannot promote themselves.
Staff cannot create organizations.
Staff cannot create administrators.
Staff lifecycle is controlled by Admin authority.
```

This is an important authorization boundary and should be enforced server-side in the Worker, not merely hidden in the UI.

The final Cloudflare implementation should therefore have two authentication paths:

```
                    ┌─────────────────────┐
                    │       Supabase         │
                    │ Google identity     │
                    │ so.muzaff@gmail.com │
                    └──────────┬──────────┘
                               │
                               ▼
                       All-Father session
                               │
                     Organization/Admin
                         provisioning
                               │
              ┌────────────────┴────────────────┐
              ▼                                 ▼
       Admin username/email              Staff management
             + password                   by Admin only
              │                                 │
              └──────────────┬──────────────────┘
                             ▼
                         ONYX D1
```

This also means the Cloudflare Worker should expose separate operations conceptually like:

```
POST /api/auth/Supabase
    → Supabase token
    → verify All-Father
    → issue ONYX All-Father session

POST /api/auth/login
    → username/email + password
    → Admin/Staff lookup in D1
    → issue ONYX session

POST /api/auth/refresh
    → refresh ONYX session

POST /api/auth/logout
    → revoke ONYX session
```

Then authorization must distinguish at least:

```
ALL_FATHER
ORGANIZATION_ADMIN
STAFF
```

with organization/tenant boundaries applied to Admin and Staff sessions.

lock this in as the ONYX security model before making further migration changes:

**Supabase/Google → All-Father only.**  
**ONYX username/email + password → Admins and Staff.**  
**All-Father → Organizations + Admin provisioning.**  
**Admins → Staff provisioning/removal.**  
**D1 → authoritative ONYX users, roles, organization membership and authorization.**
