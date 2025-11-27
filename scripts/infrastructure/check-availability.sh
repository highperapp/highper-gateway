#!/bin/bash
# Infrastructure Availability Checker
# Validates available instance types and pricing before provisioning

set -e

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Load API keys
if [ -f ".env.infrastructure" ]; then
    source .env.infrastructure
else
    echo -e "${YELLOW}Warning: .env.infrastructure not found${NC}"
    echo "Create it from .env.infrastructure.example and add your API keys"
fi

# Provider selection
PROVIDER=${1:-all}

check_vultr() {
    echo -e "${BLUE}=== Vultr Availability ===${NC}"
    echo ""

    if [ -z "$VULTR_API_KEY" ]; then
        echo -e "${RED}VULTR_API_KEY not set${NC}"
        echo "Set it in .env.infrastructure"
        return 1
    fi

    echo "Cloud Compute (vc2) - Hourly billing:"
    curl -s "https://api.vultr.com/v2/plans" \
        -H "Authorization: Bearer $VULTR_API_KEY" | \
        jq -r '.plans[] | select(.type=="vc2") |
               "\(.id): \(.vcpu_count)vCPU, \(.ram/1024)GB RAM, $\(.monthly_cost/100)/mo ($" + ((.monthly_cost/100/730) | tostring | .[0:5]) + "/hr)"' 2>/dev/null || {
                   echo -e "${RED}API call failed. Check VULTR_API_KEY${NC}"
                   return 1
               }

    echo ""
    echo "High Frequency (vhf) - Premium performance:"
    curl -s "https://api.vultr.com/v2/plans" \
        -H "Authorization: Bearer $VULTR_API_KEY" | \
        jq -r '.plans[] | select(.type=="vhf") |
               "\(.id): \(.vcpu_count)vCPU, \(.ram/1024)GB RAM, $\(.monthly_cost/100)/mo ($" + ((.monthly_cost/100/730) | tostring | .[0:5]) + "/hr)"' 2>/dev/null

    echo ""
    echo "Available Regions (showing first 10):"
    curl -s "https://api.vultr.com/v2/regions" \
        -H "Authorization: Bearer $VULTR_API_KEY" | \
        jq -r '.regions[] | "\(.id): \(.city), \(.country)"' 2>/dev/null | head -10

    echo ""
    echo -e "${GREEN}✓ Vultr API working${NC}"
}

check_hetzner() {
    echo -e "${BLUE}=== Hetzner Server Auction ===${NC}"
    echo ""
    echo "Visit: https://www.hetzner.com/sb"
    echo ""
    echo -e "${YELLOW}Manual check required (no public API for auction)${NC}"
    echo ""
    echo "Typical availability (as of 2025-11-27):"
    echo "  - AX41 (Ryzen 5 3600, 64GB): €39/mo (~\$42)"
    echo "  - AX51 (Ryzen 7 3700X, 64GB): €54/mo (~\$58)"
    echo "  - AX101 (EPYC 7502P, 128GB): €189/mo (~\$205)"
    echo ""
    echo "Steps to check:"
    echo "  1. Visit https://www.hetzner.com/sb"
    echo "  2. Filter by 'Available immediately'"
    echo "  3. Look for AX series servers"
    echo "  4. Check current prices and availability"
}

check_phoenixnap() {
    echo -e "${BLUE}=== PhoenixNAP Bare Metal ===${NC}"
    echo ""

    if [ -z "$PNAP_CLIENT_ID" ] || [ -z "$PNAP_CLIENT_SECRET" ]; then
        echo -e "${YELLOW}PNAP credentials not set${NC}"
        echo "Set PNAP_CLIENT_ID and PNAP_CLIENT_SECRET in .env.infrastructure"
        echo ""
    fi

    echo "Standard offerings (verify on website):"
    echo ""
    echo "Performance Tier (Intel Xeon):"
    echo "  - s1.c1.medium: 4c/8t, 32GB RAM, 2×500GB SSD, 1Gbps - \$189/mo"
    echo "  - s1.c1.large: 6c/12t, 64GB RAM, 2×960GB SSD, 1Gbps - \$299/mo"
    echo "  - s1.c2.xlarge: 12c/24t, 128GB RAM, 2×960GB SSD, 10Gbps - \$529/mo"
    echo ""
    echo "AMD EPYC Tier:"
    echo "  - a1.c1.large: 16c/32t, 64GB RAM, 2×960GB NVMe, 1Gbps - \$249/mo"
    echo "  - a1.c1.xlarge: 32c/64t, 128GB RAM, 2×1.92TB NVMe, 10Gbps - \$429/mo"
    echo ""
    echo "Locations: Phoenix (AZ), Ashburn (VA), Chicago (IL)"
    echo ""
    echo "Check current availability at: https://phoenixnap.com/bare-metal-cloud"
}

check_digitalocean() {
    echo -e "${BLUE}=== DigitalOcean Droplets ===${NC}"
    echo ""

    if [ -z "$DO_TOKEN" ]; then
        echo -e "${YELLOW}DO_TOKEN not set${NC}"
        echo ""
    else
        echo "Checking available droplet sizes..."
        curl -s -X GET "https://api.digitalocean.com/v2/sizes" \
            -H "Authorization: Bearer $DO_TOKEN" | \
            jq -r '.sizes[] | select(.available==true) |
                   "\(.slug): \(.vcpus)vCPU, \(.memory/1024)GB RAM, $\(.price_monthly)/mo"' 2>/dev/null | head -10 || {
                       echo -e "${RED}API call failed${NC}"
                   }
    fi

    echo ""
    echo "Popular sizes for load testing:"
    echo "  - s-2vcpu-4gb: \$24/mo"
    echo "  - s-4vcpu-8gb: \$48/mo"
    echo "  - c-4: 4vCPU dedicated, \$84/mo"
}

print_recommendations() {
    echo ""
    echo -e "${GREEN}=== Recommendations ===${NC}"
    echo ""
    echo "For your test duration:"
    echo "  < 4 hours:   Vultr Cloud Compute (\$0.12-1)"
    echo "  4-48 hours:  Vultr Cloud or High Frequency (\$1-15)"
    echo "  2-7 days:    Vultr Cloud (\$15-60)"
    echo "  > 7 days:    Hetzner Dedicated (\$40-200/month)"
    echo ""
    echo "Always validate locally (FREE) before cloud deployment!"
}

# Main execution
case $PROVIDER in
    vultr)
        check_vultr
        ;;
    hetzner)
        check_hetzner
        ;;
    phoenixnap|pnap)
        check_phoenixnap
        ;;
    digitalocean|do)
        check_digitalocean
        ;;
    all)
        check_vultr
        echo ""
        echo "=================================================="
        echo ""
        check_hetzner
        echo ""
        echo "=================================================="
        echo ""
        check_phoenixnap
        echo ""
        echo "=================================================="
        echo ""
        check_digitalocean
        ;;
    *)
        echo -e "${RED}Unknown provider: $PROVIDER${NC}"
        echo "Usage: $0 [vultr|hetzner|phoenixnap|digitalocean|all]"
        exit 1
        ;;
esac

print_recommendations
