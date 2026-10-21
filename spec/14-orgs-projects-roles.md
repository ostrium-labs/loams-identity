# 14. Organizations, projects and roles

This file covers who administers what inside the identity service (management roles), and the
coarse application roles that projects define and that can appear in tokens. Fine-grained,
resource-level permissions are not here: Loams keeps them in OpenFGA behind its `Authorizer`
([15](15-loams-integration.md)).

Most of this file is Phase 4. The management roles in section 2 are needed from Phase 1,
because the management API ([17](17-management-api.md)) is protected from the start.

## 1. Two kinds of role

| Kind | Defined by | Assigned to | Used for |
|---|---|---|---|
| Management roles | This specification (fixed set) | Users and machine users | Deciding who may call which management API operation in this service |
| Project roles | Each project (free-form keys) | Users and machine users, per project | Role claims in tokens for applications that want them |

**ORG-1.** Neither kind is a resource permission for Loams data. Loams decides access to its
resources with its own model; identity only exports membership events that Loams turns into
tuples ([15](15-loams-integration.md) section 3).

## 2. Management roles `[P1]`

### 2.1 The set

| Role | Scope object | Held through | Meaning |
|---|---|---|---|
| `instance_owner` | instance | `Instance.admins` | Everything in the instance, including managing instance owners and admins, signing keys, hosts and instance settings |
| `instance_admin` | instance | `Instance.admins` | Everything an org owner can do in every org, plus creating and removing orgs and changing instance settings; cannot manage instance owners or signing keys |
| `org_owner` | org | `Org.members` role `owner` | Everything in the org, including managing org owners |
| `org_admin` | org | `Org.members` role `admin` | Manage users, machine users, projects, applications, domains, settings and members of the org, except owners |
| `org_member` | org | `Org.members` role `member` | Read the org's name and its own memberships; nothing else |
| `project_admin` `[P4]` | project | `Project.admins` | Manage the project's applications, roles, role assignments and grants |
| `project_grant_admin` `[P4]` | project grant | `ProjectGrant.admins` | Manage role assignments of the granted org's users within the grant |

**ORG-2.** Every user can always read and change their own profile, credentials and sessions
(self-service), subject to the org's login policy. Self-service is not a role; it is checked as
"the target user is the caller".

**ORG-3.** Holders are recorded as events. `Instance.admins` is a map from user id to a set of
`instance_owner` or `instance_admin`, changed by `instance.admin_added`,
`instance.admin_roles_changed` and `instance.admin_removed` (export: audit). The org roles use the
existing `org.member_*` events ([04](04-event-catalog.md) section 3.2). `Project.admins` and
`ProjectGrant.admins` use `project.admin_added` / `project.admin_removed` and
`project_grant.admin_added` / `project_grant.admin_removed`.

**ORG-4.** An instance always has at least one `instance_owner` who is an active human user.
A command that would remove the last one fails (like DM-7 for orgs).

**ORG-5.** Instance admins are users of some org in the instance (usually the default org). Their
instance role does not make them members of other orgs; it only lets them manage those orgs.

### 2.2 Permissions

Management operations name a permission; the authorizer decides whether the caller has it on the
target object. The permission names are ours and stable, because API routes declare them
([17](17-management-api.md) API-6).

| Permission | Object | Granted to |
|---|---|---|
| `instance.read`, `instance.manage` | instance | instance_owner, instance_admin |
| `instance.manage_admins`, `instance.manage_keys` | instance | instance_owner |
| `org.create` | instance | instance_owner, instance_admin |
| `org.read` | org | org_member and above, instance_admin and above |
| `org.manage` (settings, domains, rename, deactivate) | org | org_admin and above, instance_admin and above |
| `org.manage_members` | org | org_admin (not for `owner` roles), org_owner, instance_admin and above |
| `org.manage_owners` | org | org_owner, instance_admin and above |
| `user.read`, `user.manage` | org | org_admin and above, instance_admin and above |
| `user.self` | user | the user themself |
| `project.read`, `project.manage` | project | project_admin, org_admin and above |
| `application.read`, `application.manage` | project | project_admin, org_admin and above |
| `role_assignment.manage` | project or project grant | project_admin, project_grant_admin (within the grant), org_admin of the owning org and above |
| `audit.read` | org | org_admin and above, instance_admin and above |
| `agent_session.manage` | agent user | the agent itself, its owner, org_admin and above |

"and above" means the roles above it in the table in section 2.1 for the same scope chain
(project → org → instance).

### 2.3 Evaluation

**ORG-6.** The management API calls `Authorizer::check(subject, permission, object)`. The
default implementation is the built-in evaluator, which answers from the role holders in the
event-sourced aggregates (read through the aggregates, not an asynchronous projection, ES-14).
An OpenFGA implementation of the same permissions is optional (Phase 4), with a model file
shipped in the repository and a differential test that runs one check matrix against both and
requires identical answers.

**ORG-7.** Deny by default. A permission not granted by the table is denied. An unknown
permission name is a programming error, caught by the route-declaration test (API-6).

## 3. Projects

Projects exist from Phase 1 (every org has a `default` project, [03](03-data-model.md) section
2.4) so applications have a home. Phase 4 adds roles, settings, admins and grants. In a Loams
deployment, the instance-wide project `loams` holds Loams' applications and coarse roles
([15](15-loams-integration.md) LOAMS-19a); identity projects are never mirrored to Loams projects.

**ORG-8.** A project belongs to one org for life. Its name is unique within the org.
Deactivating a project deactivates logins through its applications: authorization requests and
token requests for them fail with `unauthorized_client`, and introspection of their tokens
returns inactive. Removing it removes its applications, roles, assignments and grants
(fan-out events with `causation_id`).

## 4. Project roles `[P4]`

**ORG-9.** A project role has a `key` (`[A-Za-z0-9_.:-]{1,100}`, unique in the project,
case-sensitive), a `display_name` and an optional `group` used only for display. A project has
at most 200 roles.

**ORG-10.** Removing a role removes that key from every role assignment and every project grant
of the project (fan-out with `causation_id`). Tokens already issued keep their claims until they
expire; refresh and introspection reflect the removal at once, because they read current
assignments.

## 5. Project grants `[P4]`

A project grant shares a project with another org of the same instance, so that the other org's
users can sign in to the project's applications and the other org's admins can give their own
users a subset of the project's roles.

**ORG-11.** `ProjectGrant` aggregate:

| Field | Notes |
|---|---|
| `id`, `instance_id`, `org_id` | `org_id` is the **project's** org (the granting org) |
| `project_id` | |
| `granted_org_id` | Another active org in the same instance; at most one active grant per (project, granted org) |
| `role_keys` | The subset of project roles the granted org may assign |
| `admins` | Users of the granted org who hold `project_grant_admin` |
| `state` | `active`, `deactivated`, `removed` |

Events: `project_grant.created`, `project_grant.roles_changed`, `project_grant.deactivated`,
`project_grant.reactivated`, `project_grant.removed`, `project_grant.admin_added`,
`project_grant.admin_removed` (export: sync).

**ORG-12.** The granted org cannot change the project, its applications or its roles. It can
only assign the grant's `role_keys` to its own users.

**ORG-13.** Narrowing a grant's `role_keys` removes the dropped keys from every role assignment
made under the grant (fan-out). Deactivating or removing a grant makes its assignments
ineffective at once (checked at token issuance and introspection) and the fan-out removes them.

**ORG-14.** The grant and the granted org's state are read-checked in every token-issuing batch
for a user of the granted org (ES-10).

## 6. Role assignments `[P4]`

**ORG-15.** `RoleAssignment` aggregate:

| Field | Notes |
|---|---|
| `id`, `instance_id` | |
| `org_id` | The **user's** org |
| `project_id`, `project_org_id` | The project and its owning org |
| `project_grant_id` | Set when the user's org is not the project's org |
| `user_id` | A human or machine user |
| `role_keys` | Non-empty set |
| `state` | `active`, `removed` |

Events: `role_assignment.created`, `role_assignment.roles_changed`, `role_assignment.removed`
(export: sync). Unique per (instance, project, user) while active (inline index
`role_assignments`).

**ORG-16.** Creating or changing an assignment requires: every key exists in the project; if the
user's org is the project's org, no grant is involved; otherwise an active grant to the user's org
exists and every key is in the grant's `role_keys`. A violation fails with a validation error
naming the offending keys.

**ORG-17.** Removing a user removes their assignments (fan-out).

## 7. Login checks `[P4]`

Project settings ([03](03-data-model.md) section 2.4):

**ORG-18.** `require_org_grant_for_login`: a user may sign in to the project's applications only
if the user's org is the project's org or holds an active grant for the project.

**ORG-19.** `require_role_for_login`: a user may sign in only with at least one active role
assignment in the project.

**ORG-20.** When a check fails after the user has authenticated, the authorization request ends
with `error=access_denied` at the client's redirect URI, after the same page and delay as a user
cancelling consent (TEN-16). The session stays valid for other clients.

## 8. Role claims in tokens `[P4]`

**ORG-21.** Claim name `roles` (a private claim, ADR 0017). Shape: an object whose keys are
project ids and whose values are sorted arrays of role keys.

```json
{
  "roles": {
    "0192f7d0-1c2b-7a6e-9e1f-6a1b2c3d4e5f": ["billing.viewer", "editor"]
  }
}
```

The value describes only the token subject's own assignments. For tokens with an `act` claim,
`roles` describes the subject, never the actor.

**ORG-22.** Roles are included when either:

- the project setting `assert_roles_in_tokens` is on, for the client's own project; or
- the request has the scope `urn:loams:roles`, for the client's own project and for the project of
  every `resource_server` application in the token's audience.

The scope `urn:loams:roles` appears in discovery's `scopes_supported` once Phase 4 ships.

**ORG-23.** Where the claim appears:

| Artifact | Included |
|---|---|
| JWT access token | Yes, when ORG-22 selects it |
| ID token | Only when the scope `urn:loams:roles` was granted (an ID token is for the client; projects in the audience of the access token are not added) |
| Userinfo | Yes, when ORG-22 selects it |
| Introspection | Yes, when ORG-22 selected it at issuance, recomputed from current assignments |

**ORG-24.** Size limit: if the serialized `roles` claim would exceed 4 KiB, only the client's own
project is included; if that alone exceeds 4 KiB, the claim is omitted and
`"roles_omitted": true` is set. Resource servers that need the full set call introspection.
*Conservative choice:* a token is never silently truncated in a way that looks complete.

**ORG-25.** On refresh, roles are recomputed from current assignments. A role removed since the
last token is absent from the next one.

## 9. User metadata `[P4]`

**ORG-26.** User metadata ([03](03-data-model.md) section 2.3) is visible through the management
API to callers with `user.read`. A metadata key may be marked `expose_to_clients`; only those keys
are returned, in userinfo and the ID token, when the scope `urn:loams:metadata` is granted, under
the claim `metadata` as an object of key to string value (base64url when the value is not valid
UTF-8, with the key suffixed `#b64`).

## 10. Groups and teams (decided: Loams owns teams)

**ORG-28a.** *Owner decision, 2026-10-21:* teams are owned by Loams' control plane, which writes
the `team` tuples in OpenFGA ([15](15-loams-integration.md) section 3). This service defines no
group or team aggregate and exports no team events. Group claims from upstream identity providers
`[P5]` or SCIM `[later]` are not stored here as groups; if they are needed, they are passed to
Loams as attributes of the sign-in event and Loams maps them to teams.

## 11. Tenant scoping

**ORG-27.** A granted org's admins see only their own users' assignments for the granted project;
they never see the owning org's users or other granted orgs. The owning org's admins see all
assignments of their project, including granted orgs' users' ids and role keys, but not those
users' profiles (personal fields need `user.read` on the user's own org).

**ORG-28.** Every query in this file takes a `TenantScope` (TEN-13). The cross-tenant suite
([19](19-verification.md)) covers: assigning a role to a user of an org without a grant,
assigning a key outside the grant, reading another granted org's assignments, and signing in from
an org whose grant was deactivated.

## Sources

- Our own design for the management role set, the permission names and the claim shape.
- [RFC 7519] section 4.3 (private claim names, for `roles` and `metadata`).
- [RFC 8693] section 4.1, for the rule that the subject's roles are reported, not the actor's.
- Loams design 76 section 6 (the `org`, `project`, `team` types and the rule that application
  code checks only `can_*` relations), as a Loams-side contract.
- Zitadel public docs, observable behaviour only: a project holds roles with a key, display name
  and group; a project can be granted to another org with a subset of roles, and the granted org
  manages its own users' role assignments without changing the project; project settings can
  assert roles in tokens, require a role for login, or require the user's org to hold a grant
  ([Projects], [Granted projects]). The role names, permission names, claim name and shape,
  size limits and the "never the actor's roles" rule are ours.

[RFC 7519]: https://www.rfc-editor.org/rfc/rfc7519
[RFC 8693]: https://www.rfc-editor.org/rfc/rfc8693
[Projects]: https://zitadel.com/docs/concepts/structure/projects
[Granted projects]: https://zitadel.com/docs/concepts/structure/granted_projects
