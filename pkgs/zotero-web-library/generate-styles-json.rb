# frozen_string_literal: true

require "json"
require "csl"

source, output = ARGV
abort "usage: #{$PROGRAM_NAME} STYLES_DIRECTORY OUTPUT" unless source && output

def style_name_from_url(url)
  url&.sub(%r{/+$}, "")&.split("/")&.last
end

def parse_style(file)
  style = CSL::Style.load(file)
  info = style.info
  name = File.basename(file, ".csl")
  title_short = info.children[:"title-short"]&.to_s

  {
    "title" => style.title,
    "titleShort" => title_short,
    "name" => name,
    "dependent" => style.dependent? ? 1 : 0,
    "categories" => {
      "format" => style.citation_format&.to_s,
      "fields" => info.categories.filter_map { |category| category[:field]&.to_s }
    },
    "updated" => info.updated_at&.strftime("%Y-%m-%d %H:%M:%S"),
    "href" => "https://www.zotero.org/styles/#{name}",
    "parent" => style_name_from_url(info.independent_parent_link)
  }
rescue CSL::ParseError => error
  raise "could not parse #{file}: #{error.message}"
end

files = [
  *Dir.glob(File.join(source, "*.csl")),
  *Dir.glob(File.join(source, "dependent", "*.csl"))
].sort_by { |file| File.basename(file) }

styles = files.map { |file| parse_style(file) }
styles_by_name = styles.to_h { |style| [style.fetch("name"), style] }

resolve_format = lambda do |style, chain = []|
  format = style.dig("categories", "format")
  return format if format

  parent_name = style.fetch("parent")
  raise "dependent style #{style.fetch('name')} has no independent parent" unless parent_name
  raise "independent parent #{parent_name} is absent from the styles repository" unless styles_by_name.key?(parent_name)
  raise "independent-parent cycle: #{[*chain, style.fetch('name')].join(' -> ')}" if chain.include?(style.fetch("name"))

  resolve_format.call(styles_by_name.fetch(parent_name), [*chain, style.fetch("name")])
end

styles.each do |style|
  style.fetch("categories")["format"] = resolve_format.call(style)
  style.delete("titleShort") if style["titleShort"].nil? || style["titleShort"].empty?
  style.delete("parent")

  required = [style["title"], style["updated"], style.dig("categories", "format")]
  raise "incomplete CSL metadata for #{style.fetch('name')}" if required.any? { |value| value.nil? || value.empty? }
end

File.write(output, JSON.generate(styles))
