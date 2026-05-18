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
			key := strings.ToLower(dir)
			index[key] = append(index[key], r)
		}
	}
	return index
}

func MatchAddons(locals []*addon.Addon, remotes []RemoteAddon) []MatchedAddon {
	index := buildDirIndex(remotes)
	var matched []MatchedAddon

	for _, local := range locals {
		key := strings.ToLower(local.FolderName)
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

		best := candidates[0]
		for _, c := range candidates[1:] {
			if len(c.UIDirs) < len(best.UIDirs) {
				best = c
			}
		}

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

func isUpdateAvailable(local, remote string) bool {
	_, update, _ := classifyVersionUpdate(local, remote)
	return update
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
