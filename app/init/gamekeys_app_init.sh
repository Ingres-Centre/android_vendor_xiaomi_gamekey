#!/system/bin/sh

set -euo pipefail

CONFIG_SERVICE_COMPONENT="org.ingres.gamekeys/org.ingres.gamekeys.service.AccessibilityService"

local cur new svc
svc="${CONFIG_SERVICE_COMPONENT}"

cur="$(settings --user 0 get secure enabled_accessibility_services 2>/dev/null || true)"
if [ "${cur}" = "null" ] || [ -z "${cur}" ]; then
  new="${svc}"
elif printf ":%s:" "${cur}" | grep -Fq ":${svc}:" ; then
  new="${cur}"
else
  new="${cur}:${svc}"
fi

settings --user 0 put secure enabled_accessibility_services "${new}"
settings --user 0 put secure accessibility_enabled 1
