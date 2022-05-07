m = Map("pikpak-fuse")
m.title = translate("pikpakDrive FUSE")
m.description = translate("<a href=\"https://github.com/ykxVK8yL5L/pikpak-fuse\" target=\"_blank\">Project GitHub URL</a>")

m:section(SimpleSection).template = "pikpak-fuse/pikpak-fuse_status"

e = m:section(TypedSection, "default")
e.anonymous = true

enable = e:option(Flag, "enable", translate("Enable"))
enable.rmempty = false

username = e:option(Value, "username", translate("Username"))
username.description = translate("Username")
username.rmempty = false


password = e:option(Value, "password", translate("Password"))
password.description = translate("Password")
password.rmempty = false
password.password = true

proxy_url = e:option(Value, "proxy_url", translate("Proxy Url"))
proxy_url.description = translate("Proxy Url")
proxy_url.rmempty = true


mount_point = e:option(Value, "mount_point", translate("Mount Point"))
mount_point.default = "/mnt/pikpakDrive"

read_buffer_size = e:option(Value, "read_buffer_size", translate("Read Buffer Size"))
read_buffer_size.default = "10485760"
read_buffer_size.datatype = "uinteger"


debug = e:option(Flag, "debug", translate("Debug Mode"))
debug.rmempty = false

return m
