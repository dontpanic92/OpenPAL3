#ifndef _OPENPAL3CONFIG_HPP_
#define _OPENPAL3CONFIG_HPP_

#include <string>
#include <fstream>
#include "toml11/toml.hpp"

class OpenPal3Config
{
public:
	void Load()
	{
		this->mData = toml::parse(CONFIG_NAME);
	}

	std::string GetAssetPath()
	{
		return toml::find_or<std::string>(this->mData, "asset_path", "");
	}

	void SetAssetPath(std::string path)
	{
		this->mData.as_table()["asset_path"] = path;
	}

	void Save()
	{
		std::ofstream out(CONFIG_NAME, std::ios_base::binary);
		out << this->mData;
	}

private:
	const std::string CONFIG_NAME = "openpal3.toml";
	toml::value mData = toml::table();
};

#endif
