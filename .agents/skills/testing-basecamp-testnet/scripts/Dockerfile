FROM ubuntu:24.04
RUN apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    ca-certificates curl python3 python3-pyatspi at-spi2-core xvfb xdotool x11-utils xauth dbus-x11 \
    libopengl0 libgl1 libegl1 libxkbcommon-x11-0 libxcb-cursor0 libxcb-icccm4 \
    libxcb-keysyms1 libxcb-shape0 libxcb-xinerama0 libxcb-render-util0 \
    libxcb-image0 libxcb-randr0 libx11-xcb1 libdbus-1-3 libfontconfig1 \
    libfreetype6 libharfbuzz0b libnss3 libasound2t64 && rm -rf /var/lib/apt/lists/*
COPY LogosBasecamp-Desktop-v0.3.2-f2fae6-x86_64.AppImage /opt/basecamp.AppImage
WORKDIR /opt
RUN chmod +x basecamp.AppImage && ./basecamp.AppImage --appimage-extract >/dev/null
ENV QT_QPA_PLATFORM=xcb QT_QUICK_BACKEND=software LIBGL_ALWAYS_SOFTWARE=1
ENV QTWEBENGINE_DISABLE_SANDBOX=1
ENV QT_ACCESSIBILITY=1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
WORKDIR /lab
