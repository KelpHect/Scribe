package main

import (
	"testing"

	"Scribe/internal/esoui"
)

func TestSuppressMD5MatchesClearsFalsePositiveUpdates(t *testing.T) {
	matched := []esoui.MatchedAddon{
		{
			Remote:          &esoui.RemoteAddon{UID: "same"},
			UpdateAvailable: true,
		},
		{
			Remote:          &esoui.RemoteAddon{UID: "different"},
			UpdateAvailable: true,
		},
		{
			Remote:          &esoui.RemoteAddon{UID: "missing-remote-md5"},
			UpdateAvailable: true,
		},
		{
			Remote:          &esoui.RemoteAddon{UID: "already-current"},
			UpdateAvailable: false,
			UpdateState:     esoui.UpdateStateUpToDate,
		},
		{
			Remote:          &esoui.RemoteAddon{UID: "same-version-new-download"},
			UpdateAvailable: false,
			UpdateState:     esoui.UpdateStateUpToDate,
		},
	}

	got := suppressMD5Matches(
		matched,
		map[string]string{
			"same":                      "abc",
			"different":                 "old",
			"missing-remote-md5":        "stored",
			"already-current":           "abc",
			"same-version-new-download": "old",
		},
		map[string]string{
			"same":                      "abc",
			"different":                 "new",
			"already-current":           "abc",
			"same-version-new-download": "new",
		},
	)

	if got[0].UpdateAvailable {
		t.Fatal("matching stored and remote MD5 should suppress update")
	}
	if !got[1].UpdateAvailable {
		t.Fatal("different stored and remote MD5 should preserve update")
	}
	if !got[2].UpdateAvailable {
		t.Fatal("missing remote MD5 should preserve update")
	}
	if got[3].UpdateAvailable {
		t.Fatal("already-current match should remain not updated")
	}
	if !got[4].UpdateAvailable {
		t.Fatal("changed MD5 with matching version should become an update")
	}
	if got[4].UpdateState != esoui.UpdateStateMD5OnlyChanged {
		t.Fatalf("UpdateState = %q, want %q", got[4].UpdateState, esoui.UpdateStateMD5OnlyChanged)
	}
}
