package addon

type Addon struct {
	ID                string   `json:"id"`
	FolderName        string   `json:"folderName"`
	Title             string   `json:"title"`
	Version           string   `json:"version"`
	Author            string   `json:"author"`
	Description       string   `json:"description"`
	DependsOn         []string `json:"dependsOn"`
	OptionalDependsOn []string `json:"optionalDependsOn"`
	SavedVariables    []string `json:"savedVariables"`
	APIVersion        string   `json:"apiVersion"`
	AddOnVersion      string   `json:"addOnVersion"`
	IsLibrary         bool     `json:"isLibrary"`
	Enabled           bool     `json:"enabled"`
	Path              string   `json:"path"`
}
