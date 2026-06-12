package esoui

import (
	"path/filepath"
	"testing"
)

func TestInstallMD5RecordsRoundTrip(t *testing.T) {
	db := newCacheTestDB(t)

	if err := SaveInstallMD5(db, "uid-a", "md5-a"); err != nil {
		t.Fatalf("SaveInstallMD5 uid-a: %v", err)
	}
	if err := SaveInstallMD5(db, "uid-b", "md5-b"); err != nil {
		t.Fatalf("SaveInstallMD5 uid-b: %v", err)
	}
	got := GetInstallMD5s(db, []string{"uid-a", "uid-b", "missing"})

	if got["uid-a"] != "md5-a" {
		t.Fatalf("uid-a md5 = %q, want md5-a", got["uid-a"])
	}
	if got["uid-b"] != "md5-b" {
		t.Fatalf("uid-b md5 = %q, want md5-b", got["uid-b"])
	}
	if _, ok := got["missing"]; ok {
		t.Fatal("missing UID should not be returned")
	}
}

func TestInstallMD5RecordsIgnoreEmptyInputs(t *testing.T) {
	db := newCacheTestDB(t)

	if err := SaveInstallMD5(nil, "uid", "md5"); err != nil {
		t.Fatalf("SaveInstallMD5 nil db: %v", err)
	}
	if err := SaveInstallMD5(db, "", "md5"); err != nil {
		t.Fatalf("SaveInstallMD5 empty uid: %v", err)
	}
	if err := SaveInstallMD5(db, "uid", ""); err != nil {
		t.Fatalf("SaveInstallMD5 empty md5: %v", err)
	}
	if got := GetInstallMD5s(nil, []string{"uid"}); got != nil {
		t.Fatalf("GetInstallMD5s nil db = %#v, want nil", got)
	}
	if got := GetInstallMD5s(db, nil); got != nil {
		t.Fatalf("GetInstallMD5s nil uids = %#v, want nil", got)
	}

	var count int64
	if err := db.Model(&DBInstallRecord{}).Count(&count).Error; err != nil {
		t.Fatalf("count records: %v", err)
	}
	if count != 0 {
		t.Fatalf("record count = %d, want 0", count)
	}
}

func TestSaveInstallMD5UpdatesExistingUID(t *testing.T) {
	db, err := OpenDB(filepath.Join(t.TempDir(), "install-records.db"))
	if err != nil {
		t.Fatalf("OpenDB: %v", err)
	}
	closeTestDB(t, db)

	if err := SaveInstallMD5(db, "uid", "old"); err != nil {
		t.Fatalf("SaveInstallMD5 old: %v", err)
	}
	if err := SaveInstallMD5(db, "uid", "new"); err != nil {
		t.Fatalf("SaveInstallMD5 new: %v", err)
	}
	got := GetInstallMD5s(db, []string{"uid"})
	if got["uid"] != "new" {
		t.Fatalf("uid md5 = %q, want new", got["uid"])
	}
}
