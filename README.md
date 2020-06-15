# homebins

Binaries for $HOME.

Homebins installs binaries and scripts to your `$HOME` directory, directly from vendor, without sudo and root.

See [Install](#install) and [Usage](#usage) for more information.

## Rationale

With Go and Rust came a whole new collection of awesome commandline tools such as [ripgrep], [bat], [exa], etc.
Thanks to the generous sponsoring of build capacity on Github Actions, Travis CI, Azure Pipelines even well-established tools such as [jq] or [pandoc] can now conveniently ship their releases as binaries.

Homebins helps you download and install the latest releases of these awesome tools to your $HOME directory, so you no longer need to wait for your distribution to ship them or install compilers and build dependencies or visit dozens of Github pages for downloads.

[ripgrep]: https://github.com/BurntSushi/ripgrep
[bat]: https://github.com/sharkdp/bat
[exa]: https://github.com/ogham/exa
[pandoc]: https://pandoc.org
[jq]: https://github.com/stedolan/jq

### Use cases

- Get the latest release of bat or jq on Ubuntu LTS.
- Install xsv or ripgrep on a server you don't have root access to.
- Publish your tool without the tedious process of getting it into mainstream distributions.

### Goals

- Get command line tools like bat or ripgrep, precompiled and straight from upstream.
- Touch only $HOME, at certain places.
- Linux support, for x86_64 and perhaps other architectures.

### Non-goals

- Package and dependency management.  We only deal with binaries.
- Build software.  Maintainers should do this.
- System-wide installation.  Leave this to distributions.
- 32-bit systems.  Do these still exist?
- Support Windows or macOS.  Use [scoop] or [Homebrew].

[scoop]: https://github.com/lukesampson/scoop
[homebrew]: https://brew.sh/

## Install

1. Make sure that `git`, `curl`, `tar` and `unzip` are installed.
2. Add `~/.local/bin` to your `$PATH` and `~/.local/share/man` to your `manpath` (Ubuntu systems seem to do the latter automatically if `$PATH` is set up).
3. Download the "homebins" artifact from the [latest release].
4. `chmod a+x ./homebins`
5. `./homebins install homebins`
6. `rm ./homebins`

There's also a [dotbot] plugin at [dotbot-homebins].

[latest release]: https://github.com/lunaryorn/homebins/releases/latest
[dotbot]: https://github.com/anishathalye/dotbot
[dotbot-homebins]:  https://github.com/lunaryorn/dotbot-homebins

## Usage

```console
# List available binaries
$ homebins list
# Install bat and ripgrep
$ homebins install bat ripgrep
# List oudated binaries and update them
$ homebins outdated
$ homebins update
# Remove ripgrep again
$ homebins remove ripgrep
# Install a binary directly from a manifest file (see below)
$ homebins manifest-install my-tool.toml
```

See `homebins --help` for more information.

## Manifests

Homebins relies on manifests written in [TOML] to describe where to get a binary from and how to install it.
By default it uses manifests from the Git repo at [lunaryorn/homebin-manifests][1]; support for custom manifest repositories is planned.
It can also use manifest files directly with any of the `manifest-*` commands.

### Notes

Homebins does not keep a database of installed manifests; it simply probes all known manifests and queries the version of the installed binary.

### Write your own manifest

Manifests are a simple TOML file with some metadata and download instructions:

```toml
[info]
# The name of the utility. Must match the filename (i.e. jq.toml)
name = "jq"
# The version of the tool
version = "1.6"
# The URL of the website or Github repo
url = "https://github.com/stedolan/jq"
# The license(s), as SPDX license expression (see below)
license = "MIT"

# How to check whether the this manifest is installed
[discover]
# The binary file to check for in ~/.local/bin
binary = "jq"
# The arguments to invoke the binary with to make it print its version
version_check.args = ["--version"]
# A regular expression to extract the version number from the output.
# Must have a single capturing group containing only the version number.
version_check.pattern = "jq-(\\d\\S+)"

# One or more installation instructions: This manifest requires two downloads
# to install.
[[install]]
# The URL to download
download = "https://github.com/stedolan/jq/releases/download/jq-1.6/jq-linux64"
# A blake2 checksum to verify the download.  We also support other checksums;
# prefer the one provided by the vendor, or blake2 if the vendor doesn't offer checksums.
checksums.b2 = "d08b0756d6a6c021c20610f184de2117827d4aeb28ce87a245a1fc6ee836ef42a3ffd3a31811ea4360361d4a63d6729baf328ac024a68545974de9f6b709733c"
# checksums.sha512 = ""
# checksums.sha256 = ""
# checksums.sha1 = ""
# Directly install the downloaded file as a binary named "jq".
# This copies the file to ~/.local/bin/jq.
# The "name" is optional; if missing it defaults to the filename of the URL.
name = "jq"
type = "bin"

# Another file to download; this time it's an archive.
[[install]]
download = "https://github.com/stedolan/jq/releases/download/jq-1.6/jq-1.6.tar.gz"
checksums.b2 = "c9be1314e9d027247de63492ee362e996ef85faf45a47ee421cad95ebde9188bff8d3fc7db64e717ab922e1052f3b1c1500f5589fc5b2199ab66effb000e442d"
# The file to install from the archive.
files = [
    # Install the entry jq-1.6/jq.1.prebuilt as manpage in section 1, named jq.1
    # This copies jq-1.6/jq.1.prebuilt from the archive to
    # ~/.local/share/man/man1/jq.1
    # Again "name" is optional and defaults to the filename of the "source".
    { source = "jq-1.6/jq.1.prebuilt", name = "jq.1", type = "man", section = 1 }
    # Homebins also supports fish completions: The following would copy
    # jq.fish to ~/.config/fish/completions/jq.fish but jq doesn't include fish
    # completion.
    # { source = "jq-1.6/jq.fish", type = "completion", shell = "fish" }
]
```

The `info.license` field uses [SPDX license expressions][spdx].

See [lunaryorn/homebin-manifests][1] for more examples.

[TOML]: https://github.com/toml-lang/toml
[1]: https://github.com/lunaryorn/homebin-manifests
[spdx]: https://spdx.dev/spdx-specification-21-web-version/#h.jxpfx0ykyb60

## License

Copyright (c) 2020 Sebastian Wiesner <sebastian@swsnr.de>

This Source Code Form is subject to the terms of the Mozilla Public
License, v. 2.0. If a copy of the MPL was not distributed with this
file, You can obtain one at <http://mozilla.org/MPL/2.0/>.
