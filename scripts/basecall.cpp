using namespace std;

#include <boost/json/src.hpp>
#include <iostream>
#include <string>
#include <vector>


// JSON stream parser - should be faster than normal parser
boost::json::value read_json( std::istream& is, std::error_code& ec )
{
    boost::json::stream_parser p;
    std::string line;

    while( std::getline( is, line ) && !line.empty())
    {
        p.write( line, ec );
        if( ec )
            return nullptr;
    }
    p.finish( ec );
    if( ec )
        return nullptr;

    return p.release();
}

int main()
{

    std::error_code ec;
    boost::json::object req;

    std::string req_id;
    std::vector<uint16_t> req_data;

    boost::json::value data = read_json(std::cin, ec);
    if( data == nullptr ) {
        std::cout << "Failed to parse basecall request: " << ec.message() << std::endl;
    } else {
        req_id = data.at("id").as_string();
        req_data = boost::json::value_to<std::vector<uint16_t>>(data.at("data"));

        std::cout << req_id << std::endl;
    } 
        
 
}
