Pod::Spec.new do |s|
  s.name             = 'mobile_core'
  s.version          = '1.0.0'
  s.summary          = 'ONYX Rust mobile core'
  s.description      = 'Static Rust core and FFI surface for ONYX mobile.'
  s.homepage         = 'https://example.invalid/onyx'
  s.license          = { :type => 'Proprietary', :text => 'Internal ONYX platform component' }
  s.author           = { 'ONYX' => 'engineering@example.invalid' }
  s.source           = { :path => '.' }
  s.platform         = :ios, '15.0'
  s.vendored_frameworks = 'Frameworks/MobileCore.xcframework'
  s.preserve_paths   = 'Frameworks/MobileCore.xcframework'
end
