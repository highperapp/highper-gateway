# Vultr Cloud Load Test - Session Summary

**Date**: November 27, 2025
**Claude Session Usage**: ~50% remaining
**Status**: Deployment in progress

## What Was Created

### Vultr Cloud Instances (ACTIVE)

| Role | IP | Plan | Specs | Cost |
|------|----|----|-------|------|
| **Proxy** | 140.82.13.133 | vhp-12c-24gb-amd | 12 cores, 24GB RAM | $0.20/hr |
| **Backend-1** | 107.191.43.194 | vhf-8c-32gb | 8 cores, 32GB RAM | $0.26/hr |
| **Backend-2** | 45.63.8.188 | vhf-8c-32gb | 8 cores, 32GB RAM | $0.26/hr |
| **Backend-3** | 207.246.92.132 | vhf-8c-32gb | 8 cores, 32GB RAM | $0.26/hr |
| **Generator-1** | 207.148.18.64 | vhp-12c-24gb-amd | 12 cores, 24GB RAM | $0.20/hr |

**Total Cost**: ~$1.18/hour
**Test ID**: load-test-test-20251127-111338-15229

## Test Configuration

- **Scenario**: Layer 7 HTTP Load Balancer with Reverse Proxy
- **Target RPS**: 50,000 (validation test - reduced from 600K)
- **Test Duration**: 60 seconds
- **Load Balancer**: Round-robin
- **Features**: Health checks, Keep-alive, Compression (gzip/br), Rate limiting, Connection pooling

## Current Activity

1. ✅ Provisioned 5 Vultr cloud instances
2. ✅ Created deployment scripts
3. ✅ Created cleanup script
4. ⏳ Waiting for SSH access
5. ⏳ Will deploy binaries
6. ⏳ Will run load test

## Scripts Created

### 1. Provisioning
```bash
./scripts/loadtest/vultr-cloud/provision.sh
```

### 2. Deployment (currently running)
```bash
./scripts/loadtest/vultr-cloud/deploy-minimal.sh
```

### 3. Cleanup
```bash
./scripts/loadtest/vultr-cloud/cleanup-partial.sh
```

## Issues Encountered

1. **OS ID mismatch**: Cloud instances require OS ID 1743 (not 387) ✅ Fixed
2. **Plan availability**: vhf-16c-58gb not available in ewr region ✅ Fixed
3. **Spending limit**: Hit Vultr account monthly fee limit after 5 servers
   - Missing: 2 additional generators
   - **Solution**: Can test with 1 generator (sufficient for validation)

## Next Steps

### After Current Deployment:
1. **If successful**: View results, validate Layer 7 functionality
2. **Cleanup**: Run `./scripts/loadtest/vultr-cloud/cleanup-partial.sh`
3. **Request Vultr limit increase** for full testing in next session

### Files to Check:
- `deploy-minimal.log` - Deployment progress
- `vultr-cloud-provision.log` - Provisioning log

## Cost Management

**Current hourly cost**: $1.18/hour
**Vultr credits remaining**: ~$249 (after $1 spent)
**Recommendation**: Clean up after validation test to conserve credits

## Configuration Files

### Environment (.env)
```bash
# Vultr Cloud (verified available in ewr)
VULTR_CLOUD_REGION="ewr"
VULTR_CLOUD_PROXY_PLAN="vhp-12c-24gb-amd"
VULTR_CLOUD_BACKEND_PLAN="vhf-8c-32gb"
VULTR_CLOUD_GENERATOR_PLAN="vhp-12c-24gb-amd"
```

### Scenario Config
- File: `/tmp/scenario-02.conf`
- Backends: 3x round-robin
- Max connections: 1M
- Rate limit: 200K RPS (burst: 50K)
- Connection pool: 100 min / 5K max idle / 500K max open

## Manual Cleanup Commands

If scripts fail, delete instances manually:

```bash
export API_KEY="SRW6Z5G2IRA4EF3VHIZZIXRNMEZQW73FJALA"

curl -X DELETE -H "Authorization: Bearer $API_KEY" \
  https://api.vultr.com/v2/instances/a196d2d8-cd70-4c95-9ebd-965611d660e8

curl -X DELETE -H "Authorization: Bearer $API_KEY" \
  https://api.vultr.com/v2/instances/ece8cc70-7dd4-4c50-8352-3407f320f304

curl -X DELETE -H "Authorization: Bearer $API_KEY" \
  https://api.vultr.com/v2/instances/92aaaa4f-464a-47d7-802b-63969e07f798

curl -X DELETE -H "Authorization: Bearer $API_KEY" \
  https://api.vultr.com/v2/instances/19fa5a5d-3e3a-4943-9d80-f1cd6e46cc7a

curl -X DELETE -H "Authorization: Bearer $API_KEY" \
  https://api.vultr.com/v2/instances/5f7ffbe6-1629-4749-bc48-a8e1688486ac
```

## For Next Session

1. **Request Vultr spending limit increase** (to allow 7 instances)
2. **Alternative**: Test on different provider (Hetzner Cloud cheaper, faster provisioning)
3. **Full scenario testing**: Once limit increased, run full 600K RPS tests
4. **Additional scenarios**: Test scenarios 03-15 sequentially

## Files Modified/Created This Session

- `scripts/loadtest/vultr-cloud/provision.sh` - Cloud provisioning
- `scripts/loadtest/vultr-cloud/deploy-minimal.sh` - Minimal deployment
- `scripts/loadtest/vultr-cloud/cleanup-partial.sh` - Instance cleanup
- `.env` - Added cloud instance configuration
- `VULTR_CLOUD_TEST_STATUS.md` - This file

---

**Note**: Deployment running in background. Check `deploy-minimal.log` for progress.
