# frozen_string_literal: true

# iOS code-signing helpers for Fastlane (match + gym export).
# Ported from AstroFusion shared mobile tooling.
#
# Root cause this solves:
#   Multiple "Apple Distribution: … (TEAM)" certs with the same CN in the
#   keychain make exportArchive pick a cert that is NOT embedded in the
#   match App Store profile → "doesn't include signing certificate".
#
# Usage (from Fastfile.shared / app Fastfiles):
#   ctx = IosSigningHelpers.resolve_match_signing_context(bundle_ids)
#   IosSigningHelpers.ensure_match_signing!(bundle_ids, team_id: "…")
#   opts = IosSigningHelpers.build_appstore_export_options(...)

require 'open3'
require 'base64'
require 'set'
require 'tempfile'

module IosSigningHelpers
  DISTRIBUTION_CN_PREFIX = 'Apple Distribution'

  module_function

  # ---------------------------------------------------------------------------
  # Parsing / pure helpers (unit-testable)
  # ---------------------------------------------------------------------------

  # Parse `security find-identity -v -p codesigning` output into identities.
  # Returns array of { sha1:, name: }
  def parse_codesigning_identities(output)
    identities = []
    output.to_s.each_line do |line|
      m = line.match(/^\s*\d+\)\s+([0-9A-Fa-f]{40})\s+"([^"]+)"/)
      next unless m

      identities << { sha1: m[1].upcase, name: m[2] }
    end
    identities
  end

  def distribution_identities_from_list(identities, team_id: nil)
    identities.select do |id|
      next false unless id[:name].to_s.start_with?(DISTRIBUTION_CN_PREFIX)
      next true if team_id.nil? || team_id.to_s.empty?

      id[:name].include?(team_id.to_s)
    end
  end

  # Extract PEM from a .mobileprovision via `security cms -D`.
  def extract_profile_plist_xml(profile_path, cms_runner: method(:run_security_cms_d))
    raise ArgumentError, "Profile not found: #{profile_path}" unless File.file?(profile_path.to_s)

    cms_runner.call(profile_path)
  end

  # From decoded mobileprovision plist XML, return first DeveloperCertificates cert as DER bytes.
  def developer_certificate_der_from_plist(plist_xml)
    m = plist_xml.to_s.match(
      %r{<key>DeveloperCertificates</key>\s*<array>\s*<data>\s*([A-Za-z0-9+/=\s]+)\s*</data>}m
    )
    raise 'No DeveloperCertificates in provisioning profile plist' unless m

    Base64.decode64(m[1].gsub(/\s+/, ''))
  end

  def certificate_sha1_from_der(der_bytes, openssl_runner: method(:run_openssl_sha1))
    openssl_runner.call(der_bytes).to_s.strip.upcase.gsub(/[^0-9A-F]/, '')
  end

  def certificate_cn_from_der(der_bytes, openssl_runner: method(:run_openssl_subject))
    subject = openssl_runner.call(der_bytes).to_s
    cn = subject[/\/CN=([^\/]+)/, 1]
    cn = subject[/CN\s*=\s*([^,\n\/]+)/, 1] if cn.nil? || cn.empty?
    raise "Could not parse CN from certificate subject: #{subject}" if cn.nil? || cn.empty?

    cn.strip
  end

  def orphan_distribution_sha1s(all_distribution:, keep_sha1s:)
    keep = Set.new(Array(keep_sha1s).map { |s| s.to_s.upcase })
    all_distribution.map { |i| i[:sha1].upcase }.reject { |sha| keep.include?(sha) }.uniq
  end

  def auto_prune_enabled?(env = ENV)
    val = env.fetch('AF_AUTO_PRUNE_ORPHAN_DISTRIBUTION_CERTS', '1')
    !%w[0 false no off].include?(val.to_s.strip.downcase)
  end

  def sigh_profile_path(bundle_id, env = ENV)
    env["sigh_#{bundle_id}_appstore_profile-path"]
  end

  def sigh_certificate_name(bundle_id, env = ENV)
    env["sigh_#{bundle_id}_appstore_certificate-name"]
  end

  def resolve_match_signing_context(
    bundle_ids,
    team_id: nil,
    env: ENV,
    identities: nil,
    profile_reader: method(:read_profile_cert_info)
  )
    ids = Array(bundle_ids).flatten.compact.map(&:to_s).reject(&:empty?).uniq
    raise ArgumentError, 'bundle_ids required' if ids.empty?

    profile_paths = {}
    missing_profiles = []
    keep_sha1s = []
    cns = []

    ids.each do |bid|
      path = sigh_profile_path(bid, env)
      if path.nil? || path.empty? || !File.file?(path)
        missing_profiles << bid
        next
      end

      profile_paths[bid] = path
      info = profile_reader.call(path)
      keep_sha1s << info[:sha1]
      cns << info[:cn]
    end

    keep_sha1s = keep_sha1s.compact.map { |s| s.to_s.upcase }.uniq
    cns = cns.compact.uniq

    all_ids = identities || list_codesigning_identities
    dist = distribution_identities_from_list(all_ids, team_id: team_id)
    orphans = orphan_distribution_sha1s(all_distribution: dist, keep_sha1s: keep_sha1s)

    signing_cn =
      cns.first ||
      ids.map { |b| sigh_certificate_name(b, env) }.compact.find { |n| !n.to_s.empty? } ||
      dist.first&.dig(:name)

    codesigning_identity = signing_cn || keep_sha1s.first

    {
      bundle_ids: ids,
      keep_sha1s: keep_sha1s,
      signing_certificate: signing_cn,
      codesigning_identity: codesigning_identity,
      codesigning_identity_sha1: keep_sha1s.first,
      profile_paths: profile_paths,
      missing_profiles: missing_profiles,
      distribution_identities: dist,
      orphan_sha1s: orphans
    }
  end

  def build_appstore_export_options(
    team_id:,
    provisioning_profiles:,
    signing_certificate: nil,
    extra: {}
  )
    opts = {
      method: 'app-store',
      provisioningProfiles: provisioning_profiles || {},
      signingStyle: 'manual',
      teamID: team_id,
      stripSwiftSymbols: true,
      uploadSymbols: true
    }
    opts[:signingCertificate] = signing_certificate if signing_certificate && !signing_certificate.to_s.empty?
    opts.merge(extra || {})
  end

  def ensure_match_signing!(
    bundle_ids,
    team_id: nil,
    env: ENV,
    ui: nil,
    auto_prune: nil,
    identities: nil,
    deleter: method(:delete_certificate_by_sha1!),
    identity_lister: method(:list_codesigning_identities),
    profile_reader: method(:read_profile_cert_info)
  )
    ctx = resolve_match_signing_context(
      bundle_ids,
      team_id: team_id,
      env: env,
      identities: identities,
      profile_reader: profile_reader
    )

    if ctx[:missing_profiles].any?
      msg = <<~MSG
        Missing App Store provisioning profiles for: #{ctx[:missing_profiles].join(', ')}
        Run match first (sync_certs / sync_project_certs) so sigh_* env vars are set.
        Expected env: sigh_<bundle_id>_appstore_profile-path
      MSG
      fail_or_raise(ui, msg)
    end

    if ctx[:keep_sha1s].empty?
      fail_or_raise(ui, 'Could not extract Distribution certificate SHA-1 from match profiles.')
    end

    present = ctx[:distribution_identities].map { |i| i[:sha1].upcase }
    missing_keep = ctx[:keep_sha1s].reject { |s| present.include?(s) }
    if missing_keep.any?
      fail_or_raise(
        ui,
        "Match profile certificate(s) not installed in keychain: #{missing_keep.join(', ')}\n" \
        'Re-run match (sync_certs) so the Distribution cert is imported.'
      )
    end

    prune = auto_prune.nil? ? auto_prune_enabled?(env) : auto_prune
    if ctx[:orphan_sha1s].any?
      if prune
        ui_message(ui, "🔐 Found #{ctx[:orphan_sha1s].size} orphan Apple Distribution cert(s) not in match profiles — pruning keychain…")
        ctx[:orphan_sha1s].each do |sha|
          ui_message(ui, "   Removing orphan cert SHA-1: #{sha}")
          ok = deleter.call(sha)
          ui_message(ui, "   delete-certificate #{sha}: #{ok ? 'ok' : 'FAILED'}")
        end

        deleted = ctx[:orphan_sha1s].map { |s| s.to_s.upcase }
        rescanned =
          if identities
            identities.reject { |i| deleted.include?(i[:sha1].to_s.upcase) }
          else
            identity_lister.call
          end
        remaining = distribution_identities_from_list(rescanned, team_id: team_id)
        still = orphan_distribution_sha1s(all_distribution: remaining, keep_sha1s: ctx[:keep_sha1s])
        if still.any?
          fail_or_raise(
            ui,
            <<~MSG
              Failed to remove orphan Apple Distribution cert(s) from keychain: #{still.join(', ')}
              Delete manually (Keychain Access → My Certificates), then re-run:
                security delete-certificate -Z #{still.first} login.keychain-db
            MSG
          )
        end
        ui_success(ui, '✅ Orphan Distribution certificates removed from keychain.')
        ctx[:orphan_sha1s] = []
        ctx[:distribution_identities] = remaining
      else
        fail_or_raise(
          ui,
          <<~MSG
            Multiple Apple Distribution certificates for team #{team_id || '?'} in keychain.
            Match profiles use: #{ctx[:keep_sha1s].join(', ')}
            Orphan SHA-1s:      #{ctx[:orphan_sha1s].join(', ')}

            Xcode exportArchive often picks the wrong cert when CNs collide, causing:
              Provisioning profile "match AppStore …" doesn't include signing certificate "Apple Distribution: …"

            Fix (recommended):
              export AF_AUTO_PRUNE_ORPHAN_DISTRIBUTION_CERTS=1   # default
              # or delete orphans manually:
              #{ctx[:orphan_sha1s].map { |s| "security delete-certificate -Z #{s} login.keychain-db" }.join("\n  ")}

            Then re-run the deploy / gym lane.
          MSG
        )
      end
    else
      ui_message(ui, "🔐 Distribution signing OK — match cert #{ctx[:keep_sha1s].join(', ')}")
    end

    ui_message(ui, "   signingCertificate: #{ctx[:signing_certificate]}") if ctx[:signing_certificate]
    ui_message(ui, "   codesigning_identity: #{ctx[:codesigning_identity]}") if ctx[:codesigning_identity]

    ctx
  end

  def list_codesigning_identities
    out, _status = Open3.capture2('security', 'find-identity', '-v', '-p', 'codesigning')
    parse_codesigning_identities(out)
  end

  def read_profile_cert_info(profile_path)
    xml = extract_profile_plist_xml(profile_path)
    der = developer_certificate_der_from_plist(xml)
    {
      sha1: certificate_sha1_from_der(der),
      cn: certificate_cn_from_der(der)
    }
  end

  def run_security_cms_d(profile_path)
    out, err, status = Open3.capture3('security', 'cms', '-D', '-i', profile_path.to_s)
    raise "security cms -D failed for #{profile_path}: #{err}" unless status.success?

    out
  end

  def run_openssl_sha1(der_bytes)
    out, err, status = Open3.capture3('openssl', 'x509', '-inform', 'DER', '-noout', '-fingerprint', '-sha1', stdin_data: der_bytes)
    raise "openssl sha1 failed: #{err}" unless status.success?

    out[/=\s*([0-9A-Fa-f:]+)/, 1] || out
  end

  def run_openssl_subject(der_bytes)
    out, err, status = Open3.capture3('openssl', 'x509', '-inform', 'DER', '-noout', '-subject', stdin_data: der_bytes)
    raise "openssl subject failed: #{err}" unless status.success?

    out
  end

  def delete_certificate_by_sha1!(sha1)
    sha = sha1.to_s.upcase
    login_db = File.expand_path('~/Library/Keychains/login.keychain-db')
    login_legacy = File.expand_path('~/Library/Keychains/login.keychain')
    keychains = [login_db, 'login.keychain-db', login_legacy, 'login.keychain'].uniq

    keychains.each do |kc|
      next if kc.include?('/') && !File.exist?(kc)

      _out, _err, status = Open3.capture3('security', 'delete-certificate', '-Z', sha, kc)
      return true if status.success?
    end

    _out, _err, status = Open3.capture3('security', 'delete-certificate', '-Z', sha)
    status.success?
  end

  def gym_manual_signing_xcargs(team_id:, codesigning_identity:, extra: 'CODE_SIGN_ALLOW_ENTITLEMENTS_MODIFICATION=YES')
    identity = "\"#{codesigning_identity.to_s.gsub('"', '\\"')}\""
    parts = [
      'CODE_SIGN_STYLE=Manual',
      "CODE_SIGN_IDENTITY=#{identity}",
      "CODE_SIGN_IDENTITY[sdk=iphoneos*]=#{identity}",
      "DEVELOPMENT_TEAM=#{team_id}"
    ]
    parts << extra if extra && !extra.to_s.empty?
    parts.join(' ')
  end

  def fail_or_raise(ui, message)
    if ui && ui.respond_to?(:user_error!)
      ui.user_error!(message)
    else
      raise message
    end
  end

  def ui_message(ui, msg)
    ui.message(msg) if ui && ui.respond_to?(:message)
  end

  def ui_success(ui, msg)
    if ui && ui.respond_to?(:success)
      ui.success(msg)
    elsif ui && ui.respond_to?(:message)
      ui.message(msg)
    end
  end
end
