module("luci.controller.Pikpak-fuse", package.seeall)

function index()
	if not nixio.fs.access("/etc/config/Pikpak-fuse") then
		return
	end

	local page
	page = entry({"admin", "services", "Pikpak-fuse"}, alias("admin", "services", "Pikpak-fuse", "client"), _("PikpakDrive FUSE"), 10) -- 首页
	page.dependent = true
	page.acl_depends = { "luci-app-Pikpak-fuse" }

	entry({"admin", "services", "Pikpak-fuse", "client"}, cbi("Pikpak-fuse/client"), _("Settings"), 10).leaf = true -- 客户端配置
	entry({"admin", "services", "Pikpak-fuse", "log"}, form("Pikpak-fuse/log"), _("Log"), 30).leaf = true -- 日志页面

	entry({"admin", "services", "Pikpak-fuse", "status"}, call("action_status")).leaf = true
	entry({"admin", "services", "Pikpak-fuse", "logtail"}, call("action_logtail")).leaf = true
end

function action_status()
	local e = {}
	e.running = luci.sys.call("pidof Pikpak-fuse >/dev/null") == 0
	e.application = luci.sys.exec("Pikpak-fuse --version")
	luci.http.prepare_content("application/json")
	luci.http.write_json(e)
end

function action_logtail()
	local fs = require "nixio.fs"
	local log_path = "/var/log/Pikpak-fuse.log"
	local e = {}
	e.running = luci.sys.call("pidof Pikpak-fuse >/dev/null") == 0
	if fs.access(log_path) then
		e.log = luci.sys.exec("tail -n 100 %s | sed 's/\\x1b\\[[0-9;]*m//g'" % log_path)
	else
		e.log = ""
	end
	luci.http.prepare_content("application/json")
	luci.http.write_json(e)
end
