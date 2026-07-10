# Zurfur Domain Crate

The `domain` crate is the core of Zurfur's ports-and-adapters architecture. It contains all domain logic without any I/O dependencies, making it a pure representation of the business rules and entities.

## Architecture Overview

Zurfur follows a ports-and-adapters pattern where the `domain` layer acts as the center of the architecture:

- **Domain Layer**: Contains entities, value objects, and traits (ports) that define how the system interacts with the outside world
- **Adapter Layers**: Implement the domain traits for different storage backends (`adapter-pg`, `adapter-atproto`, `adapter-mem`)
- **Composition Root**: The `api` crate composes the system by selecting which adapters to use

## Modules

### 1. elements/

The `elements` module contains the core domain entities and value objects that represent the business concepts in Zurfur:

- **Account** - Represents platform-custodied entities with their own sovereign identity
- **User** - A recognized visitor who has been provisioned by the system
- **Role** - Member's rank inside an account, defining what authority they have
- **Commission** - The basic unit of work in the system, representing a collaborative project
- **Handle** - A validated, normalized atproto-style account handle
- **Did** - A decentralized identifier used for authentication and identity
- And many more specialized elements like `profile`, `maturity`, `invitation` etc.

### 2. ports/

The `ports` module contains traits (the "seams") that define how the domain interacts with external systems:

- **Database** - Interface for database transactions using UnitOfWork pattern
- **UserStore/UserWrites** - Stores and writes user data
- **AccountStore/AccountWrites** - Manages accounts and memberships
- **PublicRecords** - Interface for writing to public atproto repositories
- **DidMinter** - Mints decentralized identifiers (DIDs) for accounts
- **KeyStore** - Manages private keys for DID custody
- **PlcOperationLog** - Tracks operations performed on DIDs

### 3. datetime/

Contains the single clock type the domain speaks in, ensuring time is handled consistently across all domains.

## Design Principles

### Transactional Operations

All writes to the private store go through a transactional `UnitOfWork` pattern which ensures data integrity and atomicity of multi-step operations.

### Identity Rules

- Visitor identity precedes the platform (ZMVP-9, DESIGN/User)
- Account identity is platform-custodied (ZMVP-14, DESIGN/Account)  
- Role granting follows strict rules where only Owners and Admins can grant roles (DESIGN/Roles)

### Domain-Driven Design

- All domain elements are documented in the DESIGN wiki (linked in comments)
- The domain layer is completely independent - no adapters can depend on it
- The architecture allows for easy swapping of storage mechanisms via adapter pattern

## Key Features

### 1. Account Management

- Accounts have sovereign identities with DIDs
- Membership system with roles (Owner, Admin, Manager, Member)
- Invitation and acceptance flow for joining accounts
- Soft-delete functionality to preserve data integrity

### 2. Commission System  

- Independent work units with lifecycle tracking
- Content tree structure managed separately via `node` submodule
- Visibility levels (Private, Listed, Public)
- Deadline and status tracking

### 3. Authentication

- DID-based authentication following atproto principals
- Profile fetching from PDS (Public Data Boundary)
- Session management through authenticator traits

## Usage Pattern

1. **Reading from domain** - Use the store ports (like `AccountStore`, `UserStore`)
2. **Writing to domain** - Use the write ports (like `AccountWrites`, `UserWrites`)
3. **Transactions** - All writes must go through `Database::begin()` and `UnitOfWork`
4. **Error Handling** - Domain-specific errors are wrapped in `anyhow::Result` with contextual information

## Testing

The domain crate includes tests that validate:

- Fact contract compliance for commissioned facts
- Lifecycle step transitions and validations
- Role granting rules
- Various entity validation logic
