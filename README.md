# Just-Wifi-Menu

Menu de gerenciamento de Wi-Fi leve, rápido e customizável para **Hyprland / Wayland**, escrito em **Rust** com **GTK4**, **Relm4**, **gtk4-layer-shell** e **zbus** (comunicação direta com o NetworkManager via D-Bus).

---

## Funcionalidades

* **Overlay nativo Wayland (`wlr-layer-shell`):** abre ancorado no canto da barra (ex: `top-right` perto do ícone da Waybar) sem entrar no tiling, fechando com `Esc` ou ao clicar fora.
* **Atualização em tempo real via D-Bus:** escuta sinais do `NetworkManager` (`AccessPointAdded`, `AccessPointRemoved`, `ActiveAccessPoint`, `WirelessEnabled`, `State`) sem polling.
* **Gerenciamento completo no dia a dia:**
  * Ligar/desligar Wi-Fi e solicitar scan.
  * Conectar a redes salvas ou novas (com campo de senha inline), desconectar e esquecer perfis.
  * **Adicionar rede oculta** manualmente (`Aberta`, `WPA/WPA2-PSK`, `WPA3-SAE`).
* **Detalhes contextuais ao expandir uma rede:**
  * Frequência, intensidade do sinal (%) e tipo de segurança.
  * IPv4 e Gateway com botão de copiar IP (quando conectada).
  * Visualização/cópia de senha sob demanda (`GetSecrets`), toggle de **Auto-reconectar** e **Esquecer** (quando salva).
* **Aba Redes Salvas:** lista todos os perfis salvos (mesmo fora de alcance), com última conexão, segurança, auto-reconexão e remoção.
* **Aba Personalização ao vivo:**
  * Idioma (**Português / English**).
  * Paletas rápidas (*Catppuccin*, *Tokyo*, *Nord*, *Gruvbox*), color picker inline + código hex para cor principal e secundária.
  * Raio das bordas, espaçamento, padding das linhas, posição na tela (`anchor`) e margens.
  * Comportamento (*fechar ao conectar*, *fechar ao clicar fora*, *sinal mínimo*).
  * Seletor de aplicativo compatível para abrir `config.toml` / `style.css` e botão de **Recarregar CSS**.

---

## Dependências (Arch / CachyOS)

* `networkmanager`
* `gtk4`
* `gtk4-layer-shell`
* `rust` / `cargo` (para compilar)

```bash
sudo pacman -S gtk4 gtk4-layer-shell networkmanager rust
```

---

## Como compilar e executar

```bash
# Executar em modo desenvolvimento (abre a GUI)
cargo run

# Abrir ancorado no canto superior direito com margens
cargo run -- gui --anchor top-right --margin-top 10 --margin-right 10

# Compilar binário otimizado
cargo build --release
./target/release/Just-Wifi-Menu
```

### Instalação no Arch / CachyOS via `PKGBUILD`

```bash
cd packaging
makepkg -si
```

Após instalar, o comando `just-wifi-menu` fica disponível em `/usr/bin/just-wifi-menu`.

---

## Uso via CLI

Além da interface gráfica (`gui`, padrão), o binário também oferece subcomandos no terminal:

```bash
Just-Wifi-Menu list
Just-Wifi-Menu scan
Just-Wifi-Menu toggle <on|off>
Just-Wifi-Menu connect <ssid> [senha]
Just-Wifi-Menu disconnect
Just-Wifi-Menu forget <ssid>
Just-Wifi-Menu watch
```

Para logs de depuração:

```bash
RUST_LOG=debug cargo run
```

---

## Integração com Waybar e Hyprland

Exemplo de módulo `network` na Waybar para abrir/fechar ao clicar no ícone:

```jsonc
"network": {
    "format-wifi": "󰤨 {essid}",
    "format-disconnected": "󰤭",
    "on-click": "pkill -x just-wifi-menu || just-wifi-menu gui --anchor top-right --margin-top 10 --margin-right 10"
}
```

Exemplo de atalho no `hyprland.conf`:

```ini
bind = $mainMod, W, exec, pkill -x just-wifi-menu || just-wifi-menu gui --anchor top-right
```

---

## Configuração e Tema

Na primeira execução da GUI, os arquivos padrão são criados automaticamente em:

* `~/.config/just-wifi-menu/config.toml` — preferências de posição, cores, margens, idioma e comportamento.
* `~/.config/just-wifi-menu/style.css` — folha de estilos GTK4 CSS para customização avançada (dica: use `GTK_DEBUG=interactive cargo run` para inspecionar widgets e classes CSS ao vivo).

---

## Estrutura do Projeto

```text
src/
├── main.rs              # CLI (clap), inicialização de logs (tracing) e entrada GUI/CLI
├── i18n.rs              # Traduções tipadas (PT / EN)
├── config/
│   ├── mod.rs           # Leitura/escrita de config.toml e CSS dinâmico
│   └── default_style.css
├── dbus/
│   ├── mod.rs           # Conexão ao System Bus
│   ├── proxies.rs       # Proxies tipados (#[zbus::proxy]) para NetworkManager
│   └── wifi.rs          # WifiManager, operações D-Bus e testes unitários
└── ui/
    ├── mod.rs           # WifiApp (Relm4), layer-shell, stack de páginas e worker tokio
    ├── network_row.rs   # Linha expansível de rede disponível
    └── saved_row.rs     # Linha expansível de rede salva
```

---

## Referências

* [D-Bus Specification](https://www.freedesktop.org/wiki/Software/dbus/)
* [NetworkManager D-Bus API](https://networkmanager.dev/docs/api/latest/)
* [Relm4](https://relm4.org/)
