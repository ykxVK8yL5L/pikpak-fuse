include $(TOPDIR)/rules.mk

PKG_NAME:=pikpak-fuse
PKG_VERSION:=0.1.13
PKG_RELEASE:=$(AUTORELEASE)

PKG_LICENSE:=MIT
PKG_MAINTAINER:=ykxVK8yL5L <https://github.com/ykxVK8yL5L>

PKG_LIBC:=musl
ifeq ($(ARCH),arm)
  PKG_LIBC:=musleabi

  ARM_CPU_FEATURES:=$(word 2,$(subst +,$(space),$(call qstrip,$(CONFIG_CPU_TYPE))))
  ifneq ($(filter $(ARM_CPU_FEATURES),vfp vfpv2),)
    PKG_LIBC:=musleabihf
  endif
endif

PKG_ARCH=$(ARCH)
ifeq ($(ARCH),i386)
  PKG_ARCH:=i686
endif

PKG_SOURCE:=pikpak-fuse-v$(PKG_VERSION).$(PKG_ARCH)-unknown-linux-$(PKG_LIBC).tar.gz
PKG_SOURCE_URL:=https://github.com/ykxVK8yL5L/pikpak-fuse/releases/download/v$(PKG_VERSION)/
PKG_HASH:=skip

include $(INCLUDE_DIR)/package.mk

define Package/pikpak-fuse
  SECTION:=multimedia
  CATEGORY:=Multimedia
  DEPENDS:=+fuse-utils
  TITLE:=FUSE for pikpak
  URL:=https://github.com/ykxVK8yL5L/pikpak-fuse
endef

define Package/pikpak-fuse/description
  FUSE for pikpak.
endef

define Package/pikpak-fuse/conffiles
/etc/config/pikpak-fuse
endef

define Download/sha256sum
  FILE:=$(PKG_SOURCE).sha256
  URL_FILE:=$(FILE)
  URL:=$(PKG_SOURCE_URL)
  HASH:=skip
endef
$(eval $(call Download,sha256sum))

define Build/Prepare
	mv $(DL_DIR)/$(PKG_SOURCE).sha256 .
	cp $(DL_DIR)/$(PKG_SOURCE) .
	shasum -a 256 -c $(PKG_SOURCE).sha256
	rm $(PKG_SOURCE).sha256 $(PKG_SOURCE)

	tar -C $(PKG_BUILD_DIR)/ -zxf $(DL_DIR)/$(PKG_SOURCE)
endef

define Build/Compile
	echo "pikpak-fuse using precompiled binary."
endef

define Package/pikpak-fuse/install
	$(INSTALL_DIR) $(1)/usr/bin
	$(INSTALL_BIN) $(PKG_BUILD_DIR)/pikpak-fuse $(1)/usr/bin/pikpak-fuse
	$(INSTALL_DIR) $(1)/etc/init.d
	$(INSTALL_BIN) ./files/pikpak-fuse.init $(1)/etc/init.d/pikpak-fuse
	$(INSTALL_DIR) $(1)/etc/config
	$(INSTALL_CONF) ./files/pikpak-fuse.config $(1)/etc/config/pikpak-fuse
endef

$(eval $(call BuildPackage,pikpak-fuse))
