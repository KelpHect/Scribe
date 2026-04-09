package scanner

import (
	"bufio"
	"os"
	"path/filepath"
	"regexp"
	"strings"

	"Scribe/internal/addon"
)

var colorCodeRe = regexp.MustCompile(`\|c[0-9A-Fa-f]{6}([^|]*)\|r`)

func StripColorCodes(s string) string {
	return colorCodeRe.ReplaceAllString(s, "$1")
}

func ParseAddonFile(path string) (*addon.Addon, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()

	dir := filepath.Dir(path)
	folderName := filepath.Base(dir)

	a := &addon.Addon{
		ID:         folderName,
		FolderName: folderName,
		Path:       dir,
		Enabled:    true,
	}

	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if !strings.HasPrefix(line, "##") {
			continue
		}
		line = strings.TrimPrefix(line, "## ")
		key, value, ok := splitHeader(line)
		if !ok {
			continue
		}
		value = strings.TrimSpace(value)
		value = StripColorCodes(value)
		switch strings.ToLower(key) {
		case "title":
			a.Title = value
		case "version":
			a.Version = value
		case "author":
			a.Author = value
		case "description":
			a.Description = value
		case "dependson":
			a.DependsOn = parseList(value)
		case "pcdependson":
			a.DependsOn = append(a.DependsOn, parseList(value)...)
		case "consoledependson":
		case "optionaldependson":
			a.OptionalDependsOn = parseList(value)
		case "savedvariables":
			a.SavedVariables = parseList(value)
		case "apiversion":
			a.APIVersion = value
		case "addonversion":
			a.AddOnVersion = value
		case "islibrary":
			a.IsLibrary = strings.EqualFold(value, "true") || value == "1"
		}
	}

	if err := scanner.Err(); err != nil {
		return nil, err
	}

	if a.Title == "" {
		a.Title = folderName
	}

	return a, nil
}

func splitHeader(line string) (string, string, bool) {
	idx := strings.Index(line, ":")
	if idx == -1 {
		return "", "", false
	}
	return line[:idx], line[idx+1:], true
}

func parseList(value string) []string {
	if value == "" {
		return nil
	}
	var result []string
	for _, part := range strings.Fields(value) {
		clean := strings.TrimSpace(part)
		if clean != "" && !strings.HasPrefix(clean, ";") {
			result = append(result, clean)
		}
	}
	if len(result) == 0 {
		return nil
	}
	return result
}
