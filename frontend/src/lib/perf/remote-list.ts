import { compareVersionStrings } from '$lib/utils';
import type { GameVersion } from '$lib/services/esoui-service';

export function getLatestCompatibility(
  compatibilities: GameVersion[] | null | undefined
): GameVersion | null {
  let latest: GameVersion | null = null;

  for (const compatibility of compatibilities ?? []) {
    if (!compatibility.version) continue;
    if (!latest || compareVersionStrings(compatibility.version, latest.version) > 0) {
      latest = compatibility;
    }
  }

  return latest;
}
