package esoui

import (
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync/atomic"
	"testing"
	"time"
)

func TestClientInitGlobalConfig(t *testing.T) {
	tests := []struct {
		name       string
		globalJSON func(baseURL string) string
		wantErr    string
	}{
		{
			name: "active ESO config",
			globalJSON: func(baseURL string) string {
				return fmt.Sprintf(`{"Active":{"Status":"1"},"GAMES":[{"GameID":"ESO","GameConfig":%q,"Active":{"Status":"1"}}]}`, baseURL+"/game")
			},
		},
		{
			name: "inactive global config",
			globalJSON: func(baseURL string) string {
				return fmt.Sprintf(`{"Active":{"Status":"0"},"GAMES":[{"GameID":"ESO","GameConfig":%q,"Active":{"Status":"1"}}]}`, baseURL+"/game")
			},
			wantErr: "MMOUI service is inactive",
		},
		{
			name: "inactive ESO game",
			globalJSON: func(baseURL string) string {
				return fmt.Sprintf(`{"Active":{"Status":"1"},"GAMES":[{"GameID":"ESO","GameConfig":%q,"Active":{"Status":"0"}}]}`, baseURL+"/game")
			},
			wantErr: "ESO game is inactive",
		},
		{
			name: "missing ESO game",
			globalJSON: func(baseURL string) string {
				return fmt.Sprintf(`{"Active":{"Status":"1"},"GAMES":[{"GameID":"OTHER","GameConfig":%q,"Active":{"Status":"1"}}]}`, baseURL+"/game")
			},
			wantErr: "ESO game config URL not found",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var server *httptest.Server
			server = httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				switch r.URL.Path {
				case "/global":
					w.Header().Set("Content-Type", "application/json")
					_, _ = w.Write([]byte(tt.globalJSON(server.URL)))
				case "/game":
					_, _ = w.Write([]byte(`{"APIFeeds":{"FileList":"files","FileDetails":"details","CategoryList":"categories","ListFiles":"list"}}`))
				default:
					http.NotFound(w, r)
				}
			}))
			defer server.Close()

			client := newFixtureClient(server, server.URL+"/global")
			err := client.Init()
			if tt.wantErr != "" {
				if err == nil || !strings.Contains(err.Error(), tt.wantErr) {
					t.Fatalf("Init error = %v, want containing %q", err, tt.wantErr)
				}
				return
			}
			if err != nil {
				t.Fatalf("Init: %v", err)
			}
			feeds := client.FeedURLs()
			if feeds == nil || feeds.FileList != "files" || feeds.FileDetails != "details" || feeds.Categories != "categories" || feeds.ListFiles != "list" {
				t.Fatalf("FeedURLs = %#v", feeds)
			}
		})
	}
}

func TestClientGetJSONRetryNonOKAndMalformedJSON(t *testing.T) {
	var retryAttempts int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/retry":
			if atomic.AddInt32(&retryAttempts, 1) == 1 {
				http.Error(w, "try again", http.StatusBadGateway)
				return
			}
			_, _ = w.Write([]byte(`{"ok":true}`))
		case "/not-found":
			http.NotFound(w, r)
		case "/malformed":
			_, _ = w.Write([]byte(`{nope`))
		default:
			http.NotFound(w, r)
		}
	}))
	defer server.Close()

	client := newFixtureClient(server, server.URL+"/global")
	var got struct {
		OK bool `json:"ok"`
	}
	if err := client.getJSON(server.URL+"/retry", &got); err != nil {
		t.Fatalf("retry getJSON: %v", err)
	}
	if !got.OK || retryAttempts != 2 {
		t.Fatalf("retry result ok=%v attempts=%d, want ok and 2 attempts", got.OK, retryAttempts)
	}

	if err := client.getJSON(server.URL+"/not-found", &got); err == nil || !strings.Contains(err.Error(), "returned 404") {
		t.Fatalf("non-200 error = %v, want 404 failure", err)
	}
	if err := client.getJSON(server.URL+"/malformed", &got); err == nil || !strings.Contains(err.Error(), "decode JSON") {
		t.Fatalf("malformed JSON error = %v, want decode failure", err)
	}
}

func TestClientFetchesAndConvertsAPIResponses(t *testing.T) {
	dateMillis := int64(1710000000000)
	var detailsPath string
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/files":
			writeJSON(t, w, []apiRemoteAddon{{
				UID:               "1",
				CategoryID:        "cat",
				UIName:            "Addon",
				UIAuthorName:      "Author",
				UIDate:            dateMillis,
				UIVersion:         "1.2.3",
				UIDirs:            []string{"Addon"},
				UIFileInfoURL:     "https://example/addon",
				UIDownloadTotal:   "123",
				UIDownloadMonthly: "45",
				UIFavoriteTotal:   "6",
				UIIMGThumbs:       []string{"thumb"},
				UIIMGs:            []string{"image"},
				Compatabilities:   []GameVersion{{Version: "10", Name: "ESO"}},
				Siblings:          []string{"sib"},
			}})
		case "/categories":
			writeJSON(t, w, []apiCategory{{
				UICATID:        "cat",
				UICATTitle:     "Category",
				UICATICON:      "icon",
				UICATFileCount: "99",
				UICATParentIDs: []string{"root", "parent"},
			}})
		case "/details/1,2.json":
			detailsPath = r.URL.Path
			writeJSON(t, w, []apiRemoteAddonDetails{{
				apiRemoteAddon: apiRemoteAddon{UID: "1", UIName: "Addon", UIDirs: []string{"Addon"}},
				UIMD5:          "md5",
				UIFileName:     "addon.zip",
				UIDownload:     "https://example/download",
				UIHitCount:     "111",
				UIPending:      "1",
				UICatID:        "cat",
			}})
		default:
			http.NotFound(w, r)
		}
	}))
	defer server.Close()

	client := newFixtureClient(server, server.URL+"/global")
	client.feedURLs = &APIFeeds{
		FileList:    server.URL + "/files",
		FileDetails: server.URL + "/details/",
		Categories:  server.URL + "/categories",
	}

	addons, err := client.FetchAddonList()
	if err != nil {
		t.Fatalf("FetchAddonList: %v", err)
	}
	if len(addons) != 1 {
		t.Fatalf("addons = %d, want 1", len(addons))
	}
	wantDate := time.UnixMilli(dateMillis).UTC().Format("2006-01-02")
	if addons[0].UIDate != wantDate || addons[0].UIDownloadTotal != 123 || addons[0].UIDownloadMonthly != 45 || addons[0].UIFavoriteTotal != 6 {
		t.Fatalf("converted addon = %+v, want parsed date/counts", addons[0])
	}

	cats, err := client.FetchCategories()
	if err != nil {
		t.Fatalf("FetchCategories: %v", err)
	}
	if len(cats) != 1 || cats[0].ParentID != "root" || cats[0].Count != 99 || len(cats[0].ParentIDs) != 2 {
		t.Fatalf("converted categories = %+v", cats)
	}

	details, err := client.FetchAddonDetails([]string{"1", "2"})
	if err != nil {
		t.Fatalf("FetchAddonDetails: %v", err)
	}
	if detailsPath != "/details/1,2.json" {
		t.Fatalf("details path = %q, want /details/1,2.json", detailsPath)
	}
	if len(details) != 1 || details[0].UIMD5 != "md5" || details[0].UIHitCount != 111 || !details[0].UIPending {
		t.Fatalf("converted details = %+v", details)
	}
}

func newFixtureClient(server *httptest.Server, bootstrap string) *Client {
	return &Client{
		http:          server.Client(),
		bootstrapURL:  bootstrap,
		retryBaseWait: 0,
	}
}

func writeJSON(t *testing.T, w http.ResponseWriter, value any) {
	t.Helper()
	w.Header().Set("Content-Type", "application/json")
	if err := json.NewEncoder(w).Encode(value); err != nil {
		t.Fatalf("write JSON: %v", err)
	}
}
