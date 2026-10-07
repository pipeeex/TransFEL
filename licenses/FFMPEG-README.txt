FFmpeg
======

TransFEL bundles an unmodified FFmpeg executable to decode the H.264 video
stream sent by the Android application. FFmpeg runs as a separate process and
is invoked through the command line; it is not linked into TransFEL.

Version:      ffmpeg version 2026-09-02-git-9fc8c785e2-essentials_build
Git commit:   9fc8c785e2
Build:        "essentials" build by Gyan Doshi
Build source: https://www.gyan.dev/ffmpeg/builds/
Upstream:     https://ffmpeg.org

License:      GNU General Public License v3 (see FFMPEG-LICENSE.txt)

Source code for this exact version is available from the FFmpeg git repository
at commit 9fc8c785e2:

    https://git.ffmpeg.org/gitweb/ffmpeg.git/commit/9fc8c785e2

    git clone https://git.ffmpeg.org/ffmpeg.git
    git checkout 9fc8c785e2

On Linux and macOS TransFEL uses the FFmpeg provided by the system package
manager and bundles nothing.
