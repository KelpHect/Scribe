package esoui

import (
	"regexp"
	"strconv"
	"strings"

	"Scribe/internal/addon"
)

var versionNumRe = regexp.MustCompile(`\d+`)

const (
	UpdateStateUpToDate       = "up-to-date"
	UpdateStateRemoteNewer    = "remote-newer"
	UpdateStateLocalNewer     = "local-newer"
	UpdateStateMD5OnlyChanged = "md5-only-changed"
	UpdateStateUnknownVersion = "unknown-version"
	UpdateStateUnmatched      = "unmatched"
)

func buildDirIndex(remotes []RemoteAddon) map[string][]*RemoteAddon {
	index := make(map[string][]*RemoteAddon, len(remotes))
	for i := range remotes {
		r := &remotes[i]
		for _, dir := range r.UIDirs {
			key := normalizeRemoteDir(dir)
			if key == "" {
				continue
			}
			index[key] = append(index[key], r)
		}
	}
	return index
}

func normalizeRemoteDir(dir string) string {
	return strings.ToLower(strings.TrimSpace(dir))
}

func remoteDirCount(r *RemoteAddon) int {
	count := 0
	for _, dir := range r.UIDirs {
		if normalizeRemoteDir(dir) != "" {
			count++
		}
	}
	return count
}

func remoteDateValue(r *RemoteAddon) string {
	return strings.TrimSpace(r.UIDate)
}

func remoteIsBetterForDir(candidate, current *RemoteAddon, key string) bool {
	candidateDirCount := remoteDirCount(candidate)
	currentDirCount := remoteDirCount(current)

	candidateExactOnly := candidateDirCount == 1
	currentExactOnly := currentDirCount == 1
	if candidateExactOnly != currentExactOnly {
		return candidateExactOnly
	}

	if candidateDirCount != currentDirCount {
		return candidateDirCount < currentDirCount
	}

	candidateDate := remoteDateValue(candidate)
	currentDate := remoteDateValue(current)
	if candidateDate != currentDate {
		return candidateDate > currentDate
	}

	state, candidateNewer, _ := classifyVersionUpdate(current.UIVersion, candidate.UIVersion)
	if candidateNewer {
		return true
	}
	if state == UpdateStateLocalNewer {
		return false
	}

	if candidate.UIDownloadTotal != current.UIDownloadTotal {
		return candidate.UIDownloadTotal > current.UIDownloadTotal
	}

	candidateNameMatches := normalizeRemoteDir(candidate.UIName) == key
	currentNameMatches := normalizeRemoteDir(current.UIName) == key
	if candidateNameMatches != currentNameMatches {
		return candidateNameMatches
	}

	return candidate.UID < current.UID
}

func bestRemoteForDir(candidates []*RemoteAddon, key string) *RemoteAddon {
	if len(candidates) == 0 {
		return nil
	}

	best := candidates[0]
	for _, candidate := range candidates[1:] {
		if remoteIsBetterForDir(candidate, best, key) {
			best = candidate
		}
	}
	return best
}

func BestRemoteForDir(remotes []RemoteAddon, dir string) (RemoteAddon, bool) {
	key := normalizeRemoteDir(dir)
	if key == "" {
		return RemoteAddon{}, false
	}

	index := buildDirIndex(remotes)
	best := bestRemoteForDir(index[key], key)
	if best == nil {
		return RemoteAddon{}, false
	}
	return *best, true
}

func MatchAddons(locals []*addon.Addon, remotes []RemoteAddon) []MatchedAddon {
	index := buildDirIndex(remotes)
	var matched []MatchedAddon

	for _, local := range locals {
		key := normalizeRemoteDir(local.FolderName)
		candidates, ok := index[key]
		if !ok || len(candidates) == 0 {
			matched = append(matched, MatchedAddon{
				FolderName:    local.FolderName,
				LocalVersion:  local.Version,
				UpdateState:   UpdateStateUnmatched,
				UpdateReason:  "No ESOUI catalog entry matched this addon folder.",
				RemoteVersion: "",
			})
			continue
		}

		best := bestRemoteForDir(candidates, key)

		state, updateAvailable, reason := classifyVersionUpdate(local.Version, best.UIVersion)
		matched = append(matched, MatchedAddon{
			FolderName:      local.FolderName,
			Remote:          best,
			UpdateAvailable: updateAvailable,
			LocalVersion:    local.Version,
			RemoteVersion:   best.UIVersion,
			UpdateState:     state,
			UpdateReason:    reason,
		})
	}
	return matched
}

func classifyVersionUpdate(local, remote string) (string, bool, string) {
	local = strings.TrimSpace(local)
	remote = strings.TrimSpace(remote)
	if local == "" || remote == "" {
		return UpdateStateUnknownVersion, false, "Local or remote version is missing, so Scribe will not auto-offer an update from version text alone."
	}
	if local == remote {
		return UpdateStateUpToDate, false, "Local and ESOUI versions match."
	}

	lParts := extractVersionParts(local)
	rParts := extractVersionParts(remote)
	if len(lParts) == 0 || len(rParts) == 0 {
		return UpdateStateUnknownVersion, false, "Version text could not be compared safely."
	}

	max := len(lParts)
	if len(rParts) > max {
		max = len(rParts)
	}
	for i := 0; i < max; i++ {
		lv, rv := 0, 0
		if i < len(lParts) {
			lv = lParts[i]
		}
		if i < len(rParts) {
			rv = rParts[i]
		}
		if rv > lv {
			return UpdateStateRemoteNewer, true, "ESOUI has a newer version."
		}
		if rv < lv {
			return UpdateStateLocalNewer, false, "Local version appears newer than ESOUI."
		}
	}

	return UpdateStateUpToDate, false, "Local and ESOUI versions compare as equal."
}

func extractVersionParts(s string) []int {
	matches := versionNumRe.FindAllString(s, -1)
	parts := make([]int, 0, len(matches))
	for _, m := range matches {
		n, err := strconv.Atoi(m)
		if err == nil {
			parts = append(parts, n)
		}
	}
	return parts
}

func SearchRemote(remotes []RemoteAddon, query string) []RemoteAddon {
	if query == "" {
		return remotes
	}
	q := strings.ToLower(query)
	var results []RemoteAddon
	for _, r := range remotes {
		if strings.Contains(strings.ToLower(r.UIName), q) ||
			strings.Contains(strings.ToLower(r.UIAuthorName), q) {
			results = append(results, r)
		}
	}
	return results
}
