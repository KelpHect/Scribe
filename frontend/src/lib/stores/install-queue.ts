export function filterNewInstallUIDs(
  uids: string[],
  isActive: (_uid: string) => boolean
): string[] {
  const seen = new Set<string>();
  const unique: string[] = [];

  for (const uid of uids.map((item) => item.trim()).filter(Boolean)) {
    if (seen.has(uid) || isActive(uid)) continue;
    seen.add(uid);
    unique.push(uid);
  }

  return unique;
}

export type RetryableInstallTask = {
  uid: string;
  state: string;
};

export function filterRetryInstallUIDs(
  tasks: RetryableInstallTask[],
  isActive: (_uid: string) => boolean
): string[] {
  return filterNewInstallUIDs(
    tasks.filter((task) => task.state === 'failed').map((task) => task.uid),
    isActive
  );
}
