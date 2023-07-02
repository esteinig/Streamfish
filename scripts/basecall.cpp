using namespace std;

#include <sstream>
#include <iostream>
#include <iterator>
#include <string>
#include <vector>

// Simple text string, no dependencies
int main() {

    bool classifier = false;
    
    std::string line;
    std::string req_id;
    
    std::int32_t req_channel;
    std::int32_t req_number;

    float req_digitisation;
    float req_offset;
    float req_range;

    std::uint64_t req_sample_rate;
    
    std::vector<int16_t> req_data;  // uncalibrated signal data from byte data out of minknow

    while(std::getline( std::cin, line ) && !line.empty()){  


        if (classifier) {
             std::cout << line;  // read the line back out, this is simulating fastq input from basecaller
        } else {

            std::istringstream text_stream(line);
            
            text_stream >> req_id; 
            text_stream >> req_channel;
            text_stream >> req_number; 
            text_stream >> req_digitisation; 
            text_stream >> req_offset;
            text_stream >> req_range;
            text_stream >> req_sample_rate;

            std::vector<std::int16_t> req_data( 
                ( std::istream_iterator<std::int16_t>( text_stream ) ),
                ( std::istream_iterator<std::int16_t>() ) 
            );

            std::cout << req_id << " " << req_channel << " " << req_number << " " << req_digitisation << " " << req_offset << " " << req_range << " " << req_sample_rate << std::endl;  // for basic testing
        }
        
        std::cerr << "STDERR logging on Dori" << std::endl;

    }

}

