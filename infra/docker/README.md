# Docker

The backend and processor Dockerfiles live next to their source
(`backend/Dockerfile`, `processor/Dockerfile`) rather than here, so each
component's build context and image definition stay colocated with the
code it packages — a common, easy-to-navigate convention for small
multi-component repos. `docker-compose.yml` at the repo root wires them
together for local development; see [docs/deployment.md](../../docs/deployment.md)
for what changes in production.
