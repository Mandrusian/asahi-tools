# Screenshots

This directory is for README screenshots. To generate them:

    # GUI (requires running X11/Wayland session)
    asahi-gtk &
    # Take a screenshot with your compositor's tool:
    #   KDE:    spectacle
    #   niri:   grim ~/asahi-tools/docs/gui.png
    #   Hypr:   grim ~/asahi-tools/docs/gui.png
    # Save to docs/gui.png

    # TUI (in a terminal)
    asahi-tui
    # Screenshot the terminal window
    # Save to docs/tui.png

    # Trackpad visualizer (requires sudo)
    sudo asahi-trackpad-viz
    # Screenshot
    # Save to docs/trackpad.png

Then in README.md, add under the intro:

    ![Asahi Settings](docs/gui.png)
    ![Asahi Tools TUI](docs/tui.png)

