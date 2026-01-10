# SSH Key Issue - Load Test Deployment

## Problem

Vultr cloud instances were created **without SSH keys**, making them inaccessible via SSH.

**Error**: `Permission denied (publickey,password)`

## Root Cause

The `provision.sh` script creates instances without specifying `sshkey_id` parameter:

```json
{
  "region": "ewr",
  "plan": "vhp-12c-24gb-amd",
  "os_id": 1743,
  "label": "proxy-...",
  "hostname": "proxy",
  "tag": "..."
  // MISSING: "sshkey_id": ["<key-id>"]
}
```

## Solution

### Step 1: Get your Vultr SSH Key ID

```bash
curl -s -H "Authorization: Bearer SRW6Z5G2IRA4EF3VHIZZIXRNMEZQW73FJALA" \
  "https://api.vultr.com/v2/ssh-keys" | jq '.ssh_keys[]'
```

If no keys exist, create one:

```bash
# Generate key if needed
ssh-keygen -t ed25519 -f ~/.ssh/vultr_loadtest -N ""

# Upload to Vultr
curl -X POST "https://api.vultr.com/v2/ssh-keys" \
  -H "Authorization: Bearer SRW6Z5G2IRA4EF3VHIZZIXRNMEZQW73FJALA" \
  -H "Content-Type: application/json" \
  -d "{
    \"name\": \"loadtest-key\",
    \"ssh_key\": \"$(cat ~/.ssh/vultr_loadtest.pub)\"
  }"
```

### Step 2: Fix provision.sh

Add SSH key to instance creation (line ~40-55 in provision.sh):

```bash
response=$(curl -s -X POST "${VULTR_API_URL}/instances" \
    -H "Authorization: Bearer ${VULTR_API_KEY}" \
    -H "Content-Type: application/json" \
    -d "{
        \"region\": \"${VULTR_CLOUD_REGION}\",
        \"plan\": \"${VULTR_CLOUD_PROXY_PLAN}\",
        \"os_id\": ${VULTR_OS_ID},
        \"label\": \"proxy-${TAG}\",
        \"hostname\": \"proxy\",
        \"tag\": \"${TAG}\",
        \"sshkey_id\": [\"${VULTR_SSH_KEY_ID}\"]
    }")
```

### Step 3: Add to .env

```bash
# Add this to .env
VULTR_SSH_KEY_ID="your-key-id-from-step-1"
```

## Current Status

**5 Vultr instances running**: $1.18/hour
**Cannot access**: No SSH keys configured
**Deployment**: Stuck waiting for SSH

## Recommendations

### Option 1: Clean up now (RECOMMENDED)
```bash
# Stop deployment
# Clean up instances to stop billing
./scripts/loadtest/vultr-cloud/cleanup-partial.sh
```

**Why**: Instances costing $1.18/hour with no way to access them

### Option 2: Access via Vultr Console (complex)
1. Log into Vultr dashboard
2. Access each instance via web console
3. Manually add SSH key to `/root/.ssh/authorized_keys`
4. Resume deployment

**Why not recommended**: Time-consuming, error-prone

## For Next Session

1. ✅ Upload SSH key to Vultr
2. ✅ Get SSH key ID
3. ✅ Update `provision.sh` to include `sshkey_id`
4. ✅ Update `.env` with `VULTR_SSH_KEY_ID`
5. ✅ Re-provision with SSH access
6. ✅ Complete load test

## Files to Update

### `scripts/loadtest/vultr-cloud/provision.sh`
Add `sshkey_id` to all instance creation calls (3 places):
- provision_proxy() - line ~46
- provision_backends() - line ~83
- provision_generators() - line ~123

### `.env`
```bash
VULTR_SSH_KEY_ID="<your-key-id>"
```

## Cost Impact

**Time wasted**: ~15 minutes at $1.18/hour = ~$0.30
**Claude usage**: ~56% used
**Vultr credits remaining**: ~$249

## Lesson Learned

Always include SSH key configuration in cloud instance provisioning API calls. Most cloud providers require this for security.

---

**Created**: 2025-11-27
**Issue**: SSH keys not configured during provisioning
**Status**: Instances need cleanup and re-provisioning
