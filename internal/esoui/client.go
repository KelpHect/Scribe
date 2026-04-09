package esoui

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strconv"
	"strings"
	"time"
)

const (
	bootstrapURL = "https://api.mmoui.com/v3/globalconfig.json"
	esoGameKey   = "ESO"
	httpTimeout  = 30 * time.Second

	maxRetries    = 3
	retryBaseWait = 1 * time.Second
)

type apiGlobalActive struct {
	Status string `json:"Status"`
}

type apiGlobalGame struct {
	GameID     string          `json:"GameID"`
	GameConfig string          `json:"GameConfig"`
	Active     apiGlobalActive `json:"Active"`
}

type apiGlobalConfig struct {
	Active apiGlobalActive `json:"Active"`
	Games  []apiGlobalGame `json:"GAMES"`
}

type apiRemoteAddon struct {
	UID               string        `json:"UID"`
	CategoryID        string        `json:"UICATID"`
	UIName            string        `json:"UIName"`
	UIAuthorName      string        `json:"UIAuthorName"`
	UIDate            int64         `json:"UIDate"`
	UIVersion         string        `json:"UIVersion"`
	UIDirs            []string      `json:"UIDir"`
	UIFileInfoURL     string        `json:"UIFileInfoURL"`
	UIDownloadTotal   string        `json:"UIDownloadTotal"`
	UIDownloadMonthly string        `json:"UIDownloadMonthly"`
	UIFavoriteTotal   string        `json:"UIFavoriteTotal"`
	UIIMGThumbs       []string      `json:"UIIMG_Thumbs"`
	UIIMGs            []string      `json:"UIIMGs"`
	Compatabilities   []GameVersion `json:"UICompatibility"`
	Siblings          []string      `json:"UISiblings"`
}

type apiRemoteAddonDetails struct {
	apiRemoteAddon
	UIMD5             string `json:"UIMD5"`
	UIFileName        string `json:"UIFileName"`
	UIDownload        string `json:"UIDownload"`
	UIDescription     string `json:"UIDescription"`
	UIChangeLog       string `json:"UIChangeLog"`
	UIHitCount        string `json:"UIHitCount"`
	UIHitCountMonthly string `json:"UIHitCountMonthly"`
	UIDonation        string `json:"UIDonationLink"`
	UIPending         string `json:"UIPending"`
	UICatID           string `json:"UICATID"`
}

func convertAddonDetails(a apiRemoteAddonDetails) RemoteAddonDetails {
	base := convertAddon(a.apiRemoteAddon)
	hitCount, _ := strconv.ParseInt(strings.TrimSpace(a.UIHitCount), 10, 64)
	hitCountMonthly, _ := strconv.ParseInt(strings.TrimSpace(a.UIHitCountMonthly), 10, 64)
	return RemoteAddonDetails{
		RemoteAddon:       base,
		UIMD5:             a.UIMD5,
		UIFileName:        a.UIFileName,
		UIDownload:        a.UIDownload,
		UIDescription:     a.UIDescription,
		UIChangeLog:       a.UIChangeLog,
		UIHitCount:        hitCount,
		UIHitCountMonthly: hitCountMonthly,
		UIDonation:        a.UIDonation,
		UIPending:         a.UIPending == "1",
		UICatID:           a.UICatID,
	}
}

type apiCategory struct {
	UICATID        string   `json:"UICATID"`
	UICATTitle     string   `json:"UICATTitle"`
	UICATICON      string   `json:"UICATICON"`
	UICATFileCount string   `json:"UICATFileCount"`
	UICATParentIDs []string `json:"UICATParentIDs"`
}

func convertAddon(a apiRemoteAddon) RemoteAddon {
	downloads, _ := strconv.ParseInt(a.UIDownloadTotal, 10, 64)
	downloadsMonthly, _ := strconv.ParseInt(a.UIDownloadMonthly, 10, 64)
	favorites, _ := strconv.ParseInt(a.UIFavoriteTotal, 10, 64)

	uiDate := ""
	if a.UIDate > 0 {
		t := time.UnixMilli(a.UIDate)
		uiDate = t.UTC().Format("2006-01-02")
	}

	return RemoteAddon{
		UID:               a.UID,
		CategoryID:        a.CategoryID,
		UIName:            a.UIName,
		UIAuthorName:      a.UIAuthorName,
		UIDate:            uiDate,
		UIVersion:         a.UIVersion,
		UIDirs:            a.UIDirs,
		UIFileInfoURL:     a.UIFileInfoURL,
		UIDownloadTotal:   downloads,
		UIDownloadMonthly: downloadsMonthly,
		UIFavoriteTotal:   favorites,
		UIIMGThumbs:       a.UIIMGThumbs,
		UIIMGs:            a.UIIMGs,
		Compatabilities:   a.Compatabilities,
		Siblings:          a.Siblings,
	}
}

func convertCategory(c apiCategory) Category {
	count, _ := strconv.Atoi(c.UICATFileCount)
	parentID := ""
	if len(c.UICATParentIDs) > 0 {
		parentID = c.UICATParentIDs[0]
	}
	return Category{
		ID:        c.UICATID,
		Name:      c.UICATTitle,
		IconURL:   c.UICATICON,
		ParentID:  parentID,
		ParentIDs: c.UICATParentIDs,
		Count:     count,
	}
}

type Client struct {
	http       *http.Client
	gameConfig *GameConfig
	feedURLs   *APIFeeds
}

func NewClient() *Client {
	return &Client{
		http: &http.Client{Timeout: httpTimeout},
	}
}

func (c *Client) Init() error {
	gcURL, err := c.fetchGlobalConfig()
	if err != nil {
		return fmt.Errorf("fetch global config: %w", err)
	}

	feeds, err := c.fetchGameConfig(gcURL)
	if err != nil {
		return fmt.Errorf("fetch ESO game config: %w", err)
	}
	c.feedURLs = feeds
	return nil
}

func (c *Client) FeedURLs() *APIFeeds {
	return c.feedURLs
}

func (c *Client) CloseIdleConnections() {
	if c.http != nil {
		c.http.CloseIdleConnections()
	}
}

func (c *Client) fetchGlobalConfig() (string, error) {
	var cfg apiGlobalConfig
	if err := c.getJSON(bootstrapURL, &cfg); err != nil {
		return "", err
	}

	if cfg.Active.Status != "" && cfg.Active.Status != "1" {
		return "", fmt.Errorf("MMOUI service is inactive (status=%q)", cfg.Active.Status)
	}

	for _, g := range cfg.Games {
		if g.GameID == esoGameKey {

			if g.Active.Status != "" && g.Active.Status != "1" {
				return "", fmt.Errorf("ESO game is inactive on MMOUI (status=%q)", g.Active.Status)
			}
			return g.GameConfig, nil
		}
	}
	return "", fmt.Errorf("ESO game config URL not found in globalconfig (checked %d entries)", len(cfg.Games))
}

func (c *Client) fetchGameConfig(url string) (*APIFeeds, error) {
	var cfg GameConfig
	if err := c.getJSON(url, &cfg); err != nil {
		return nil, err
	}
	return &cfg.APIFeeds, nil
}

func (c *Client) FetchAddonList() ([]RemoteAddon, error) {
	if c.feedURLs == nil {
		return nil, fmt.Errorf("client not initialised, call Init() first")
	}
	var raw []apiRemoteAddon
	if err := c.getJSON(c.feedURLs.FileList, &raw); err != nil {
		return nil, fmt.Errorf("fetch addon list: %w", err)
	}
	addons := make([]RemoteAddon, len(raw))
	for i, a := range raw {
		addons[i] = convertAddon(a)
	}
	return addons, nil
}

func (c *Client) FetchAddonDetails(uids []string) ([]RemoteAddonDetails, error) {
	if c.feedURLs == nil {
		return nil, fmt.Errorf("client not initialised, call Init() first")
	}
	if len(uids) == 0 {
		return nil, nil
	}

	url := c.feedURLs.FileDetails + strings.Join(uids, ",") + ".json"
	var raw []apiRemoteAddonDetails
	if err := c.getJSON(url, &raw); err != nil {
		return nil, fmt.Errorf("fetch addon details: %w", err)
	}
	details := make([]RemoteAddonDetails, len(raw))
	for i, a := range raw {
		details[i] = convertAddonDetails(a)
	}
	return details, nil
}

func (c *Client) FetchCategories() ([]Category, error) {
	if c.feedURLs == nil {
		return nil, fmt.Errorf("client not initialised, call Init() first")
	}
	var raw []apiCategory
	if err := c.getJSON(c.feedURLs.Categories, &raw); err != nil {
		return nil, fmt.Errorf("fetch categories: %w", err)
	}
	cats := make([]Category, len(raw))
	for i, c := range raw {
		cats[i] = convertCategory(c)
	}
	return cats, nil
}

func (c *Client) getJSON(url string, v any) error {
	var lastErr error
	wait := retryBaseWait
	for attempt := 0; attempt <= maxRetries; attempt++ {
		if attempt > 0 {
			time.Sleep(wait)
			wait *= 2
		}
		resp, err := c.http.Get(url)
		if err != nil {
			lastErr = fmt.Errorf("GET %s: %w", url, err)
			continue
		}

		if resp.StatusCode >= 500 {
			resp.Body.Close()
			lastErr = fmt.Errorf("GET %s returned %d", url, resp.StatusCode)
			continue
		}
		if resp.StatusCode != http.StatusOK {
			resp.Body.Close()
			return fmt.Errorf("GET %s returned %d", url, resp.StatusCode)
		}

		err = json.NewDecoder(resp.Body).Decode(v)
		resp.Body.Close()
		if err != nil {
			return fmt.Errorf("decode JSON from %s: %w", url, err)
		}
		return nil
	}
	return lastErr
}
