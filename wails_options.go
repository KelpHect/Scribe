package main

func useCustomTitleBarForGOOS(goos string) bool {
	return goos != "linux"
}
