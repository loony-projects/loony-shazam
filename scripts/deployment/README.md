# Deployment scripts

This repository ships a working local/dev deployment (`docker-compose.yml`)
but deliberately does not include a production deploy pipeline — see
[docs/deployment.md](../../docs/deployment.md) for what a real production
deployment needs beyond this repo (target platform, secrets manager,
reverse proxy/TLS, etc.), none of which can be meaningfully scripted
without knowing the target environment. This directory is reserved for
that tooling once a deployment target is chosen (e.g. `deploy-ecs.sh`,
`deploy-k8s.sh`, or a Terraform/Pulumi entry point).
