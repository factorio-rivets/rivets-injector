<div align="center">
    <a href="https://github.com/notnotmelon/rivets">
    <picture>
        <source media="(prefers-color-scheme: dark)" srcset="https://forums.factorio.com/images/ext/20b0f93fa0933e4aa7df8592f124d153.png">
        <img alt="Rivets 🔩 - the Factorio mod loader" width="75%" style="max-width: 600px" src=".github/assets/logo-horizontal.png">
    </picture>
    </a>

[![Discord](https://img.shields.io/discord/1260754935952314418?color=lightblue&label=Community%20Chat&logo=Discord&logoColor=aqua)](https://discord.gg/xRYEZYz5WR)
[![](https://img.shields.io/badge/License-Rivets_2024-green)](https://github.com/notnotmelon/rivets/blob/master/LICENSE.md)

</div>

# Rivets injector 🔩
A small windows application that injects the `rivets.dll`.
Intended to be small, simple, and stable. Should be updated very infrequently.

Preforms the following operations:
- Searches the working directory for `factorio.exe`. Alternatively this could be manually provided via `--executable-path`
- Parses `config-path.cfg` and `config.ini` to find the factorio mods folder location. Alternatively this could be manually provided via `--mod-directory`
- Searches the mods folder for the highest possible version of `rivets_X.X.X.zip`.
- Unzips `rivets/rivets.dll` from the zip archive.
- Starts the `factorio.exe` process.
- Uses the `dll-syringe` crate to inject `rivets.dll` into factorio.
- Builds an OS pipe to copy factorio's stdout onto rivets' stdout.

![image](https://github.com/user-attachments/assets/8efcdc0f-c959-4673-bb83-124c51de4ddd)
