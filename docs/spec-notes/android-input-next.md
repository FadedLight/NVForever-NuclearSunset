# Android input additions

- [documented] The owner's `gyro-inversion` request asks for independent persisted X/Y
  signs, both off by default, without changing touch/controller inversion or recenter.
- [documented] `engine/nvforever/src/touch/gyro.rs` admits finite sensor radians after
  lifecycle, device, modal and ADS gates. The new signs belong at this one gyro-only
  output boundary, after gain and before the existing clamp.
- [documented] `engine/nvf-play/src/player.rs::mouse_look` combines device counts and
  gyro radians. Device vertical inversion must be applied before that combination;
  otherwise its final inversion also flips gyro, violating independent settings.
- [hypothesis] Physical comfort and orientation preference still require device
  acceptance. CPU route and INI tests establish sign/persistence behavior only.
