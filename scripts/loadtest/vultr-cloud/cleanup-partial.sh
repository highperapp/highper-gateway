#!/bin/bash
# Clean up partial Vultr cloud deployment
# Run this to delete the 5 active servers

VULTR_API_KEY="SRW6Z5G2IRA4EF3VHIZZIXRNMEZQW73FJALA"

echo "Deleting 5 Vultr cloud instances..."

curl -X DELETE -H "Authorization: Bearer ${VULTR_API_KEY}" \
  https://api.vultr.com/v2/instances/a196d2d8-cd70-4c95-9ebd-965611d660e8 && echo "✓ Proxy deleted"

curl -X DELETE -H "Authorization: Bearer ${VULTR_API_KEY}" \
  https://api.vultr.com/v2/instances/ece8cc70-7dd4-4c50-8352-3407f320f304 && echo "✓ Backend-1 deleted"

curl -X DELETE -H "Authorization: Bearer ${VULTR_API_KEY}" \
  https://api.vultr.com/v2/instances/92aaaa4f-464a-47d7-802b-63969e07f798 && echo "✓ Backend-2 deleted"

curl -X DELETE -H "Authorization: Bearer ${VULTR_API_KEY}" \
  https://api.vultr.com/v2/instances/19fa5a5d-3e3a-4943-9d80-f1cd6e46cc7a && echo "✓ Backend-3 deleted"

curl -X DELETE -H "Authorization: Bearer ${VULTR_API_KEY}" \
  https://api.vultr.com/v2/instances/5f7ffbe6-1629-4749-bc48-a8e1688486ac && echo "✓ Generator-1 deleted"

echo "Done! All instances deleted."
